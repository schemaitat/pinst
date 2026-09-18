//! App state machine (PAT-002): a single `App` struct holds all view state;
//! `handle_event` is the only mutator, called once per drained `AppEvent`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use tokio::sync::mpsc::UnboundedSender;

use crate::event::AppEvent;
use crate::health::{self, FileHealth, ToolHealth};
use crate::probe::{self, ProbeResult};
use crate::registry::ToolSpec;
use crate::upgrade::{self, UpgradeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Health,
    Upgrades,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Overview, Tab::Health, Tab::Upgrades];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Health => "Health",
            Tab::Upgrades => "Upgrades",
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

    pub registry: Vec<ToolSpec>,
    pub dotfiles_dir: PathBuf,
    pub home_dir: PathBuf,

    pub probes: BTreeMap<String, ProbeResult>,
    pub probes_expected: usize,
    pub probes_received: usize,

    pub health_files: Vec<FileHealth>,
    pub health_tools: Vec<ToolHealth>,
    pub health_ready: bool,

    pub upgrades: BTreeMap<String, UpgradeResult>,
    pub upgrades_loading: bool,
    pub upgrades_ever_run: bool,

    pub overview_selected: usize,

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
        registry: Vec<ToolSpec>,
        dotfiles_dir: PathBuf,
        home_dir: PathBuf,
    ) -> Self {
        let probes_expected = registry.len();
        Self {
            tab: Tab::Overview,
            should_quit: false,
            tx,
            registry,
            dotfiles_dir,
            home_dir,
            probes: BTreeMap::new(),
            probes_expected,
            probes_received: 0,
            health_files: Vec::new(),
            health_tools: Vec::new(),
            health_ready: false,
            upgrades: BTreeMap::new(),
            upgrades_loading: false,
            upgrades_ever_run: false,
            overview_selected: 0,
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
        probe::spawn_probe_tasks(&self.registry, self.tx.clone());
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Term(term_event) => self.handle_term_event(term_event),
            AppEvent::Tick => self.tick_count = self.tick_count.wrapping_add(1),
            AppEvent::Probe(result) => self.on_probe(result),
            AppEvent::Health { files, tools } => {
                self.health_files = files;
                self.health_tools = tools;
                self.health_ready = true;
                self.status = "health check complete".to_string();
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
            self.status = "running health check...".to_string();
            self.spawn_health_check();
        } else if !self.health_ready {
            self.status = format!("probing tools... ({}/{})", self.probes_received, self.probes_expected);
        }
    }

    fn spawn_health_check(&self) {
        let registry = self.registry.clone();
        let dotfiles_dir = self.dotfiles_dir.clone();
        let home_dir = self.home_dir.clone();
        let probes = self.probes.clone();
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let files = health::check_files(&dotfiles_dir, &home_dir);
            let tools = health::check_tool_paths(&registry, &probes);
            let _ = tx.send(AppEvent::Health { files, tools });
        });
    }

    pub fn refresh_upgrades(&mut self, force: bool) {
        self.upgrades_loading = true;
        self.upgrades_ever_run = true;
        self.status = "checking for upgrades...".to_string();
        upgrade::spawn_upgrade_check(
            self.registry.clone(),
            self.probes.clone(),
            self.tx.clone(),
            force,
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
    pub fn filtered_registry(&self) -> Vec<&ToolSpec> {
        match self.query_lower() {
            None => self.registry.iter().collect(),
            Some(q) => self
                .registry
                .iter()
                .filter(|t| t.name.to_lowercase().contains(&q) || t.package.to_lowercase().contains(&q))
                .collect(),
        }
    }

    pub fn filtered_health_files(&self) -> Vec<&FileHealth> {
        match self.query_lower() {
            None => self.health_files.iter().collect(),
            Some(q) => self
                .health_files
                .iter()
                .filter(|f| {
                    f.relative_path.to_lowercase().contains(&q) || f.package.to_lowercase().contains(&q)
                })
                .collect(),
        }
    }

    pub fn filtered_health_tools(&self) -> Vec<&ToolHealth> {
        match self.query_lower() {
            None => self.health_tools.iter().collect(),
            Some(q) => self
                .health_tools
                .iter()
                .filter(|t| t.tool.to_lowercase().contains(&q))
                .collect(),
        }
    }

    /// Files pinst can jump straight into an editor for: `~/.zshrc` first
    /// (the single most-edited file, per README), then every other
    /// dotfiles-tracked file discovered by the health check (TASK-015).
    pub fn editor_targets(&self) -> Vec<EditorTarget> {
        let mut targets = vec![EditorTarget {
            label: "~/.zshrc".to_string(),
            path: self.home_dir.join(".zshrc"),
        }];
        for file in &self.health_files {
            let label = format!("~/{}", file.relative_path);
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
                    self.overview_selected = 0;
                }
            }
            KeyCode::Char('/') => self.search_mode = true,
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            KeyCode::Char('1') => self.tab = Tab::Overview,
            KeyCode::Char('2') => self.tab = Tab::Health,
            KeyCode::Char('3') => self.tab = Tab::Upgrades,
            KeyCode::Down | KeyCode::Char('j') => self.move_overview_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_overview_selection(-1),
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
                self.overview_selected = 0;
            }
            KeyCode::Enter => self.search_mode = false,
            KeyCode::Backspace => {
                self.search_query.pop();
                self.overview_selected = 0;
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.overview_selected = 0;
            }
            _ => {}
        }
    }

    fn next_tab(&mut self) {
        let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
    }

    fn prev_tab(&mut self) {
        let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
    }

    fn move_overview_selection(&mut self, delta: i32) {
        if self.tab != Tab::Overview {
            return;
        }
        let len = self.filtered_registry().len() as i32;
        if len == 0 {
            return;
        }
        self.overview_selected = (self.overview_selected as i32 + delta).rem_euclid(len) as usize;
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
