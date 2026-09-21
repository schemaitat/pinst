//! App state machine (PAT-002): a single `App` struct holds all view state;
//! `handle_event` is the only mutator, called once per drained `AppEvent`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use tokio::sync::mpsc::UnboundedSender;

use crate::core::configs::{ConfigSet, FileStatus};
use crate::core::docs::Catalogue;
use crate::core::docs::page::ToolDoc;
use crate::core::docs::search as find;
use crate::core::doctor::{self, Finding};
use crate::core::manifest::{Manifest, Tool};
use crate::core::platform::Platform;
use crate::core::probe::{self, ProbeResult};
use crate::core::upgrade::{self, UpgradeResult};
use crate::event::{self, AppEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Health,
    Upgrades,
    Docs,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Overview, Tab::Health, Tab::Upgrades, Tab::Docs];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Health => "Health",
            Tab::Upgrades => "Upgrades",
            Tab::Docs => "Docs",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorTarget {
    pub label: String,
    pub path: PathBuf,
}

pub struct App {
    pub tab: Tab,
    pub should_quit: bool,
    tx: UnboundedSender<AppEvent>,

    pub registry: Vec<Tool>,
    pub manifest: Manifest,
    pub home_dir: PathBuf,

    pub probes: BTreeMap<String, ProbeResult>,
    pub probes_expected: usize,
    pub probes_received: usize,

    pub findings: Vec<Finding>,
    pub configs: Vec<FileStatus>,
    pub health_ready: bool,

    pub upgrades: BTreeMap<String, UpgradeResult>,
    pub upgrades_loading: bool,
    pub upgrades_ever_run: bool,

    pub overview_selected: usize,
    pub docs_selected: usize,

    /// The tool catalogue, loaded once at startup. An unreadable catalogue is
    /// an empty Docs tab, not a dashboard that refuses to open.
    pub catalogue: Catalogue,

    pub search_query: String,
    pub search_mode: bool,

    pub picker_open: bool,
    pub picker_selected: usize,
    pub pending_editor: Option<PathBuf>,

    pub status: String,
    pub tick_count: u64,
}

impl App {
    pub fn new(
        tx: UnboundedSender<AppEvent>,
        manifest: Manifest,
        home_dir: PathBuf,
        catalogue: Catalogue,
    ) -> Self {
        let registry = manifest.tools.clone();
        let probes_expected = registry.len();
        Self {
            tab: Tab::Overview,
            should_quit: false,
            tx,
            registry,
            manifest,
            home_dir,
            probes: BTreeMap::new(),
            probes_expected,
            probes_received: 0,
            findings: Vec::new(),
            configs: Vec::new(),
            health_ready: false,
            upgrades: BTreeMap::new(),
            upgrades_loading: false,
            upgrades_ever_run: false,
            overview_selected: 0,
            docs_selected: 0,
            catalogue,
            search_query: String::new(),
            search_mode: false,
            picker_open: false,
            picker_selected: 0,
            pending_editor: None,
            status: "probing tools...".to_string(),
            tick_count: 0,
        }
    }

    pub fn start_probing(&self) {
        let sink = event::forward_probes(self.tx.clone());
        probe::spawn_streaming(self.registry.clone(), sink, Platform::host());
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Term(term_event) => self.handle_term_event(term_event),
            AppEvent::Tick => self.tick_count = self.tick_count.wrapping_add(1),
            AppEvent::Probe(result) => self.on_probe(result),
            AppEvent::Health { findings, configs } => {
                self.findings = findings;
                self.configs = configs;
                self.health_ready = true;
                self.status = "diagnosis complete".to_string();
            }
            AppEvent::Upgrade(result) => {
                self.upgrades.insert(result.tool.clone(), result);
            }
            AppEvent::UpgradesDone => {
                self.upgrades_loading = false;
                self.status = "upgrade check complete".to_string();
            }
        }
    }

    fn on_probe(&mut self, result: ProbeResult) {
        self.probes.insert(result.tool.clone(), result);
        self.probes_received += 1;
        if self.probes_received == self.probes_expected && !self.health_ready {
            self.status = "diagnosing...".to_string();
            self.spawn_health_check();
        } else if !self.health_ready {
            self.status = format!(
                "probing tools... ({}/{})",
                self.probes_received, self.probes_expected
            );
        }
    }

    fn spawn_health_check(&self) {
        let manifest = self.manifest.clone();
        let home_dir = self.home_dir.clone();
        let probes = self.probes.clone();
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            // Diagnosis reads the filesystem, so it belongs off the async
            // runtime's worker threads.
            let Ok(set) = ConfigSet::load(&manifest, &home_dir) else {
                return;
            };
            // Same platform filtering the CLI applies via `graph::select`:
            // the TUI reports on the machine it is running on, so a tool
            // this host cannot install has nothing here to check.
            let refs: Vec<&Tool> = manifest
                .tools
                .iter()
                .filter(|t| {
                    matches!(
                        t.resolve(Platform::host()),
                        crate::core::manifest::Resolved::Supported(_)
                    )
                })
                .collect();
            let findings =
                doctor::diagnose(&refs, &probes, &set, Platform::host()).unwrap_or_default();
            let configs = set.status().unwrap_or_default();
            let _ = tx.send(AppEvent::Health { findings, configs });
        });
    }

    pub fn refresh_upgrades(&mut self, force: bool) {
        self.upgrades_loading = true;
        self.upgrades_ever_run = true;
        self.status = "checking for upgrades...".to_string();
        let sink = event::forward_upgrades(self.tx.clone());
        upgrade::spawn_streaming(
            self.registry.clone(),
            self.probes.clone(),
            sink,
            force,
            Platform::host(),
        );
    }

    fn query_lower(&self) -> Option<String> {
        if self.search_query.is_empty() {
            None
        } else {
            Some(self.search_query.to_lowercase())
        }
    }

    /// Tools matching the active search query (by name or package), or the
    /// full registry when no query is active. Backs both the Overview and
    /// Upgrades tabs so `/` filters consistently across them.
    pub fn filtered_registry(&self) -> Vec<&Tool> {
        match self.query_lower() {
            None => self.registry.iter().collect(),
            Some(q) => self
                .registry
                .iter()
                .filter(|t| {
                    t.name.to_lowercase().contains(&q)
                        || t.tags.iter().any(|tag| tag.to_lowercase().contains(&q))
                })
                .collect(),
        }
    }

    /// The Docs tab's list: ranked by the same search the CLI uses, so the
    /// two surfaces cannot disagree about what matches. With no query it is
    /// the manifest's own order, which is grouped by purpose.
    pub fn filtered_docs(&self) -> Vec<&Tool> {
        match self.query_lower() {
            None => self.registry.iter().collect(),
            Some(query) => {
                let entries: Vec<find::Entry<'_>> = self
                    .registry
                    .iter()
                    .map(|tool| find::Entry {
                        tool,
                        page: self.catalogue.get(&tool.name),
                    })
                    .collect();
                find::search(&entries, &query)
                    .into_iter()
                    .map(|hit| hit.tool)
                    .collect()
            }
        }
    }

    /// The page shown in the Docs tab's reading pane.
    pub fn selected_doc(&self) -> Option<(&Tool, Option<&ToolDoc>)> {
        let tools = self.filtered_docs();
        let index = self.docs_selected.min(tools.len().saturating_sub(1));
        let tool = *tools.get(index)?;
        Some((tool, self.catalogue.get(&tool.name)))
    }

    pub fn filtered_findings(&self) -> Vec<&Finding> {
        match self.query_lower() {
            None => self.findings.iter().collect(),
            Some(q) => self
                .findings
                .iter()
                .filter(|f| {
                    f.id.to_lowercase().contains(&q) || f.message.to_lowercase().contains(&q)
                })
                .collect(),
        }
    }

    pub fn filtered_configs(&self) -> Vec<&FileStatus> {
        match self.query_lower() {
            None => self.configs.iter().collect(),
            Some(q) => self
                .configs
                .iter()
                .filter(|c| {
                    c.path.to_lowercase().contains(&q) || c.package.to_lowercase().contains(&q)
                })
                .collect(),
        }
    }

    /// Files pinst can jump straight into an editor for: `~/.zshrc` first
    /// (the most-edited file day to day), then every other config pinst
    /// manages.
    pub fn editor_targets(&self) -> Vec<EditorTarget> {
        let mut targets = vec![EditorTarget {
            label: "~/.zshrc".to_string(),
            path: self.home_dir.join(".zshrc"),
        }];
        for file in &self.configs {
            let label = format!("~/{}", file.path);
            if label == "~/.zshrc" {
                continue;
            }
            targets.push(EditorTarget {
                label,
                path: file.target.clone(),
            });
        }
        targets
    }

    fn handle_term_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press {
            return;
        }
        if self.picker_open {
            self.handle_picker_key(key.code);
            return;
        }
        if self.search_mode {
            self.handle_search_key(key.code);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if self.search_query.is_empty() {
                    self.should_quit = true;
                } else {
                    // First Esc clears an active filter instead of quitting,
                    // so a search doesn't trap the user into an extra quit.
                    self.search_query.clear();
                    self.reset_selections();
                }
            }
            KeyCode::Char('/') => self.search_mode = true,
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            KeyCode::Char('1') => self.tab = Tab::Overview,
            KeyCode::Char('2') => self.tab = Tab::Health,
            KeyCode::Char('3') => self.tab = Tab::Upgrades,
            KeyCode::Char('4') => self.tab = Tab::Docs,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Char('e') => {
                self.picker_open = true;
                self.picker_selected = 0;
            }
            KeyCode::Char('r') if self.tab == Tab::Upgrades && !self.upgrades_loading => {
                self.refresh_upgrades(true);
            }
            _ => {}
        }
    }

    fn handle_search_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.search_mode = false;
                self.search_query.clear();
                self.reset_selections();
            }
            KeyCode::Enter => self.search_mode = false,
            KeyCode::Backspace => {
                self.search_query.pop();
                self.reset_selections();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.reset_selections();
            }
            _ => {}
        }
    }

    /// A new query is a new result set, so a selection into the old one means
    /// nothing.
    fn reset_selections(&mut self) {
        self.overview_selected = 0;
        self.docs_selected = 0;
    }

    fn next_tab(&mut self) {
        let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
    }

    fn prev_tab(&mut self) {
        let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
    }

    /// Moves the selection on whichever tab owns one.
    fn move_selection(&mut self, delta: i32) {
        match self.tab {
            Tab::Overview => {
                let len = self.filtered_registry().len() as i32;
                if len > 0 {
                    self.overview_selected =
                        (self.overview_selected as i32 + delta).rem_euclid(len) as usize;
                }
            }
            Tab::Docs => {
                let len = self.filtered_docs().len() as i32;
                if len > 0 {
                    self.docs_selected =
                        (self.docs_selected as i32 + delta).rem_euclid(len) as usize;
                }
            }
            _ => {}
        }
    }

    fn handle_picker_key(&mut self, code: KeyCode) {
        let targets = self.editor_targets();
        match code {
            KeyCode::Esc | KeyCode::Char('e') => self.picker_open = false,
            KeyCode::Down | KeyCode::Char('j') => {
                if !targets.is_empty() {
                    self.picker_selected = (self.picker_selected + 1) % targets.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !targets.is_empty() {
                    self.picker_selected =
                        (self.picker_selected + targets.len() - 1) % targets.len();
                }
            }
            KeyCode::Enter => {
                self.picker_open = false;
                if let Some(target) = targets.get(self.picker_selected) {
                    self.status = format!("opening {}...", target.label);
                    self.pending_editor = Some(target.path.clone());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest;
    use crate::core::source::Source;
    use crossterm::event::{KeyEvent, KeyModifiers};

    /// The tempdir comes back too: dropping it would delete the catalogue the
    /// app is holding.
    fn app(pages: &[(&str, &str)]) -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        for (name, body) in pages {
            std::fs::write(dir.path().join(format!("{name}.toml")), body).unwrap();
        }
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let manifest = manifest::load(None).unwrap().manifest;
        (
            dir,
            App::new(tx, manifest, PathBuf::from("/home/test"), catalogue),
        )
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_event(AppEvent::Term(Event::Key(KeyEvent::new(
            code,
            KeyModifiers::NONE,
        ))));
    }

    fn typed(app: &mut App, query: &str) {
        press(app, KeyCode::Char('/'));
        for c in query.chars() {
            press(app, KeyCode::Char(c));
        }
        press(app, KeyCode::Enter);
    }

    const RIPGREP: &str = r#"
what = "Recursive regex search across a tree."
keywords = ["grep", "search"]
status = "authored"
verified_with = "15.1.0"

[[recipes]]
cmd = "rg -n 'pattern' path/"
does = "Search a path."
"#;

    #[test]
    fn the_docs_tab_is_reachable_by_number_and_by_cycling() {
        let (_dir, mut app) = app(&[]);
        press(&mut app, KeyCode::Char('4'));
        assert_eq!(app.tab, Tab::Docs);

        // Cycling past the last tab wraps to the first.
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Overview);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.tab, Tab::Docs);
    }

    #[test]
    fn with_no_query_the_docs_list_is_the_whole_manifest_in_its_own_order() {
        let (_dir, app) = app(&[]);
        let listed: Vec<&str> = app
            .filtered_docs()
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        let declared: Vec<&str> = app.registry.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(listed, declared);
    }

    #[test]
    fn the_docs_list_is_ranked_by_the_same_search_the_cli_uses() {
        // The fixture is the CLI ranking tests' fixture: a keyword hit on
        // ripgrep's page must come first, exactly as `pinst docs search` puts
        // it first.
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        press(&mut app, KeyCode::Char('4'));
        typed(&mut app, "grep");
        assert_eq!(app.filtered_docs()[0].name, "ripgrep");
    }

    #[test]
    fn the_selected_tool_carries_its_page_when_it_has_one() {
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        press(&mut app, KeyCode::Char('4'));
        typed(&mut app, "grep");

        let (tool, page) = app.selected_doc().unwrap();
        assert_eq!(tool.name, "ripgrep");
        assert_eq!(page.unwrap().recipes.len(), 1);
    }

    #[test]
    fn a_tool_with_no_page_still_selects_and_reports_the_gap() {
        let (_dir, mut app) = app(&[]);
        press(&mut app, KeyCode::Char('4'));
        let (tool, page) = app.selected_doc().unwrap();
        assert!(!tool.name.is_empty());
        assert!(page.is_none());
    }

    #[test]
    fn a_search_that_matches_nothing_leaves_the_reading_pane_empty() {
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        press(&mut app, KeyCode::Char('4'));
        typed(&mut app, "kubernetes");
        assert!(app.filtered_docs().is_empty());
        assert!(app.selected_doc().is_none());
    }

    #[test]
    fn moving_the_selection_only_touches_the_tab_that_owns_one() {
        let (_dir, mut app) = app(&[]);
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.docs_selected, 1);
        assert_eq!(
            app.overview_selected, 0,
            "the Overview cursor must not move"
        );

        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.overview_selected, 1);
        assert_eq!(app.docs_selected, 1, "the Docs cursor must not move");
    }

    #[test]
    fn a_new_query_puts_both_cursors_back_at_the_top() {
        // A selection into the previous result set means nothing once the set
        // changes underneath it.
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.docs_selected, 2);

        typed(&mut app, "grep");
        assert_eq!(app.docs_selected, 0);
    }

    #[test]
    fn the_docs_tab_renders_the_selected_page() {
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        press(&mut app, KeyCode::Char('4'));
        typed(&mut app, "grep");

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &app)).unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("ripgrep"), "{rendered}");
        assert!(rendered.contains("rg -n"), "the recipe must be on screen");
        assert!(rendered.contains("authored"), "the page's status is shown");
    }
}
