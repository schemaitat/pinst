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
use crate::core::harness::asset as harness_asset;
use crate::core::harness::install::plan as harness_plan;
use crate::core::harness::install::receipt::{Receipt, ReceiptScope};
use crate::core::harness::install::record;
use crate::core::harness::install::state::{self as harness_state, AssetStatus};
use crate::core::harness::project::InstallRoot;
use crate::core::harness::vendor::Vendor;
use crate::core::manifest::{Manifest, Tool};
use crate::core::platform::Platform;
use crate::core::probe::{self, ProbeResult};
use crate::core::upgrade::{self, UpgradeCheck};
use crate::event::{self, AppEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Health,
    Upgrades,
    Docs,
    Harness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthFocus {
    Findings,
    Configs,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Overview,
        Tab::Health,
        Tab::Upgrades,
        Tab::Docs,
        Tab::Harness,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Health => "Health",
            Tab::Upgrades => "Upgrades",
            Tab::Docs => "Docs",
            Tab::Harness => "Harness",
        }
    }
}

/// Which harness action a confirmation modal is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessModalAction {
    Install,
    Uninstall,
}

impl HarnessModalAction {
    pub fn label(self) -> &'static str {
        match self {
            HarnessModalAction::Install => "install",
            HarnessModalAction::Uninstall => "uninstall",
        }
    }
}

/// A pending harness mutation, awaiting `Enter` or `Esc`. Named, not just a
/// bool, because the modal has to say *what* it is about to do and *how
/// many* steps that is — "install?" is not a sentence someone can disagree
/// with, "install 12 items into /repo/.claude" is.
#[derive(Debug, Clone, Copy)]
pub struct HarnessModal {
    pub scope: ReceiptScope,
    pub action: HarnessModalAction,
    pub steps: Option<usize>,
    generation: u64,
}

/// One scope's harness digest: where it installs and how each state adds up.
/// Derived from the already-classified rows so the scope summary and the
/// per-scope section headers cannot disagree about the same scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessScopeStat {
    pub scope: ReceiptScope,
    pub root: PathBuf,
    pub total: usize,
    pub installed: usize,
    pub missing: usize,
    pub drifted: usize,
    pub unmanaged: usize,
}

impl HarnessScopeStat {
    fn of(scope: ReceiptScope, root: PathBuf, rows: &[AssetStatus]) -> Self {
        let count = |pred: fn(harness_state::AssetState) -> bool| {
            rows.iter().filter(|row| pred(row.state)).count()
        };
        Self {
            scope,
            root,
            total: rows.len(),
            installed: count(|state| state.satisfied()),
            missing: count(|state| state == harness_state::AssetState::Missing),
            drifted: count(|state| {
                matches!(
                    state,
                    harness_state::AssetState::Drifted | harness_state::AssetState::Foreign
                )
            }),
            unmanaged: count(|state| state == harness_state::AssetState::Unmanaged),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorTarget {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventOutcome {
    pub redraw: bool,
    pub launch_editor: bool,
    pub quit: bool,
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
    health_generation: u64,

    pub upgrades: BTreeMap<String, UpgradeCheck>,
    pub upgrades_loading: bool,
    pub upgrades_ever_run: bool,

    pub overview_selected: usize,
    pub health_focus: HealthFocus,
    pub health_findings_selected: usize,
    pub health_configs_selected: usize,
    pub upgrades_selected: usize,
    pub docs_selected: usize,
    pub docs_scroll: u16,

    /// The tool catalogue, loaded once at startup. An unreadable catalogue is
    /// an empty Docs tab, not a dashboard that refuses to open.
    pub catalogue: Catalogue,

    pub search_query: String,
    pub search_mode: bool,

    pub picker_open: bool,
    pub picker_selected: usize,
    pub pending_editor: Option<PathBuf>,

    /// Where a project-scope harness install lands, resolved once at
    /// startup — an ancestor `.ash/`, else the git top-level, else the
    /// working directory. `home_dir` above already carries the global root.
    pub harness_project_root: PathBuf,
    pub harness_project: Vec<AssetStatus>,
    pub harness_global: Vec<AssetStatus>,
    pub harness_ready: bool,
    pub harness_selected: usize,
    /// Vertical scroll of the selected-asset preview pane, reset whenever the
    /// selection moves so a new asset always starts at the top.
    pub harness_scroll: u16,
    /// True while a spawned install/uninstall run is in flight — blocks a
    /// second `i`/`u` from opening a modal on top of one still running.
    pub harness_busy: bool,
    pub harness_modal: Option<HarnessModal>,
    harness_plan_generation: u64,

    pub status: String,
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
        // Resolution is a filesystem walk and at most one `git` subprocess,
        // done once here alongside the manifest load and catalogue load
        // `tui::run` already does synchronously before entering the loop —
        // not worth a background task for something this cheap and needed
        // immediately to compute the first frame's harness banner.
        let harness_project_root = InstallRoot::resolve(None)
            .map(|root| root.path().to_path_buf())
            .unwrap_or_else(|_| PathBuf::from("."));
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
            health_generation: 0,
            upgrades: BTreeMap::new(),
            upgrades_loading: false,
            upgrades_ever_run: false,
            overview_selected: 0,
            health_focus: HealthFocus::Findings,
            health_findings_selected: 0,
            health_configs_selected: 0,
            upgrades_selected: 0,
            docs_selected: 0,
            docs_scroll: 0,
            catalogue,
            search_query: String::new(),
            search_mode: false,
            picker_open: false,
            picker_selected: 0,
            pending_editor: None,
            harness_project_root,
            harness_project: Vec::new(),
            harness_global: Vec::new(),
            harness_ready: false,
            harness_selected: 0,
            harness_scroll: 0,
            harness_busy: false,
            harness_modal: None,
            harness_plan_generation: 0,
            status: "probing tools...".to_string(),
        }
    }

    pub fn start_probing(&self) {
        let sink = event::forward_probes(self.tx.clone());
        probe::spawn_streaming(self.registry.clone(), sink, Platform::host());
    }

    /// Loads both scopes' install state off the async runtime's worker
    /// threads — classification stats every managed path and reads every
    /// copied file to compare it, the same reasoning `spawn_health_check`
    /// already applies to config diagnosis.
    pub fn start_harness_load(&self) {
        let tx = self.tx.clone();
        let project_root = self.harness_project_root.clone();
        let global_root = self.home_dir.clone();
        tokio::task::spawn_blocking(move || {
            let source = harness_asset::resolve_source();
            let project =
                harness_state::status(&source, Vendor::Claude, &project_root).unwrap_or_default();
            let global =
                harness_state::status(&source, Vendor::Claude, &global_root).unwrap_or_default();
            let _ = tx.send(AppEvent::Harness { project, global });
        });
    }

    pub fn handle_event(&mut self, event: AppEvent) -> EventOutcome {
        let redraw = match event {
            AppEvent::Term(term_event) => self.handle_term_event(term_event),
            AppEvent::Probe(result) => {
                self.on_probe(result);
                true
            }
            AppEvent::Health {
                generation,
                findings,
                configs,
            } if generation == self.health_generation => {
                self.findings = findings;
                self.configs = configs;
                self.health_findings_selected = self
                    .health_findings_selected
                    .min(self.findings.len().saturating_sub(1));
                self.health_configs_selected = self
                    .health_configs_selected
                    .min(self.configs.len().saturating_sub(1));
                self.health_ready = true;
                self.status = "diagnosis complete".to_string();
                true
            }
            AppEvent::Health { .. } => false,
            AppEvent::Upgrade(result) => {
                self.upgrades.insert(result.tool.clone(), result);
                true
            }
            AppEvent::UpgradesDone => {
                self.upgrades_loading = false;
                self.status = "upgrade check complete".to_string();
                true
            }
            AppEvent::Harness { project, global } => {
                self.harness_project = project;
                self.harness_global = global;
                self.harness_ready = true;
                self.harness_busy = false;
                self.status = "harness state refreshed".to_string();
                true
            }
            AppEvent::HarnessPlan {
                generation,
                scope,
                action,
                steps,
            } => {
                if let Some(modal) = self.harness_modal.as_mut() {
                    if modal.generation != generation
                        || modal.scope != scope
                        || modal.action != action
                    {
                        false
                    } else if let Some(steps) = steps {
                        modal.steps = Some(steps);
                        self.status =
                            format!("harness {} ready ({} scope)", action.label(), scope.label());
                        true
                    } else {
                        self.harness_modal = None;
                        self.status = format!(
                            "harness: nothing to {} for {} scope",
                            action.label(),
                            scope.label()
                        );
                        true
                    }
                } else {
                    false
                }
            }
            AppEvent::HarnessScope { scope, rows } => {
                match scope {
                    ReceiptScope::Project => self.harness_project = rows,
                    ReceiptScope::Global => self.harness_global = rows,
                }
                self.harness_ready = true;
                self.harness_busy = false;
                self.status = format!("harness {} scope refreshed", scope.label());
                true
            }
        };
        EventOutcome {
            redraw,
            launch_editor: self.pending_editor.is_some(),
            quit: self.should_quit,
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

    fn spawn_health_check(&mut self) {
        self.health_generation = self.health_generation.wrapping_add(1);
        let generation = self.health_generation;
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
            let _ = tx.send(AppEvent::Health {
                generation,
                findings,
                configs,
            });
        });
    }

    pub fn on_editor_closed(&mut self, result: Result<(), String>) {
        self.status = match result {
            Ok(()) => "editor closed; refreshing health...".to_string(),
            Err(error) => format!("editor failed: {error}; refreshing health..."),
        };
        self.spawn_health_check();
    }

    pub fn refresh_upgrades(&mut self, force: bool) {
        self.upgrades.clear();
        self.upgrades_selected = 0;
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
                find::search_tools(&entries, &query)
            }
        }
    }

    /// The page shown in the Docs tab's reading pane, selected from a result
    /// set the caller already computed for this frame.
    pub fn selected_doc_in<'a>(
        &'a self,
        tools: &[&'a Tool],
    ) -> Option<(&'a Tool, Option<&'a ToolDoc>)> {
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

    /// Both scopes' rows, project first then global — the same order the
    /// CLI's `harness status` prints them in — tagged with which scope each
    /// row came from, and filtered by the active search query.
    pub fn filtered_harness(&self) -> Vec<(ReceiptScope, &AssetStatus)> {
        let rows = self
            .harness_project
            .iter()
            .map(|r| (ReceiptScope::Project, r))
            .chain(
                self.harness_global
                    .iter()
                    .map(|r| (ReceiptScope::Global, r)),
            );
        match self.query_lower() {
            None => rows.collect(),
            Some(q) => rows
                .filter(|(_, r)| r.name.to_lowercase().contains(&q))
                .collect(),
        }
    }

    /// Both scopes' digests, project first, for the scope summary and the
    /// section headers.
    pub fn harness_scopes(&self) -> [HarnessScopeStat; 2] {
        [
            self.harness_scope_stat(ReceiptScope::Project),
            self.harness_scope_stat(ReceiptScope::Global),
        ]
    }

    /// One scope's digest: its install root and per-state counts.
    pub fn harness_scope_stat(&self, scope: ReceiptScope) -> HarnessScopeStat {
        match scope {
            ReceiptScope::Project => HarnessScopeStat::of(
                scope,
                self.harness_project_root.clone(),
                &self.harness_project,
            ),
            ReceiptScope::Global => {
                HarnessScopeStat::of(scope, self.home_dir.clone(), &self.harness_global)
            }
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

    fn handle_term_event(&mut self, event: Event) -> bool {
        let Event::Key(key) = event else {
            return matches!(event, Event::Resize(_, _));
        };
        if key.kind != KeyEventKind::Press {
            return false;
        }
        if self.harness_modal.is_some() {
            return self.handle_harness_modal_key(key.code);
        }
        if self.picker_open {
            return self.handle_picker_key(key.code);
        }
        if self.search_mode {
            return self.handle_search_key(key.code);
        }
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                false
            }
            KeyCode::Esc => {
                if self.search_query.is_empty() {
                    self.should_quit = true;
                    false
                } else {
                    // First Esc clears an active filter instead of quitting,
                    // so a search doesn't trap the user into an extra quit.
                    self.search_query.clear();
                    self.reset_selections();
                    true
                }
            }
            KeyCode::Char('/') => {
                self.search_mode = true;
                true
            }
            KeyCode::Tab => {
                self.next_tab();
                true
            }
            KeyCode::BackTab => {
                self.prev_tab();
                true
            }
            KeyCode::Char('1') => self.select_tab(Tab::Overview),
            KeyCode::Char('2') => self.select_tab(Tab::Health),
            KeyCode::Char('3') => self.select_tab(Tab::Upgrades),
            KeyCode::Char('4') => self.select_tab(Tab::Docs),
            KeyCode::Char('5') => self.select_tab(Tab::Harness),
            KeyCode::Left | KeyCode::Right if self.tab == Tab::Health => {
                self.health_focus = match self.health_focus {
                    HealthFocus::Findings => HealthFocus::Configs,
                    HealthFocus::Configs => HealthFocus::Findings,
                };
                true
            }
            KeyCode::PageDown => self.scroll_pane(5),
            KeyCode::PageUp => self.scroll_pane(-5),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Char('e') => {
                self.picker_open = true;
                self.picker_selected = 0;
                true
            }
            KeyCode::Char('r') if self.tab == Tab::Upgrades && !self.upgrades_loading => {
                self.refresh_upgrades(false);
                true
            }
            KeyCode::Char('R') if self.tab == Tab::Upgrades && !self.upgrades_loading => {
                self.refresh_upgrades(true);
                true
            }
            KeyCode::Char('i') if self.tab == Tab::Harness && !self.harness_busy => {
                self.open_harness_modal(HarnessModalAction::Install)
            }
            KeyCode::Char('u') if self.tab == Tab::Harness && !self.harness_busy => {
                self.open_harness_modal(HarnessModalAction::Uninstall)
            }
            _ => false,
        }
    }

    fn handle_search_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.search_mode = false;
                self.search_query.clear();
                self.reset_selections();
                true
            }
            KeyCode::Enter => {
                self.search_mode = false;
                true
            }
            KeyCode::Backspace => {
                if self.search_query.pop().is_some() {
                    self.reset_selections();
                    true
                } else {
                    false
                }
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.reset_selections();
                true
            }
            _ => false,
        }
    }

    /// A new query is a new result set, so a selection into the old one means
    /// nothing.
    fn reset_selections(&mut self) {
        self.overview_selected = 0;
        self.health_findings_selected = 0;
        self.health_configs_selected = 0;
        self.upgrades_selected = 0;
        self.docs_selected = 0;
        self.docs_scroll = 0;
        self.harness_selected = 0;
        self.harness_scroll = 0;
    }

    fn next_tab(&mut self) {
        let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
    }

    fn prev_tab(&mut self) {
        let idx = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
    }

    fn select_tab(&mut self, tab: Tab) -> bool {
        if self.tab == tab {
            false
        } else {
            self.tab = tab;
            true
        }
    }

    /// Moves the selection on whichever tab owns one.
    fn move_selection(&mut self, delta: i32) -> bool {
        match self.tab {
            Tab::Overview => {
                let len = self.filtered_registry().len() as i32;
                if len > 0 {
                    let previous = self.overview_selected;
                    self.overview_selected =
                        (self.overview_selected as i32 + delta).rem_euclid(len) as usize;
                    return self.overview_selected != previous;
                }
            }
            Tab::Health => {
                let len = match self.health_focus {
                    HealthFocus::Findings => self.filtered_findings().len(),
                    HealthFocus::Configs => self.filtered_configs().len(),
                };
                let selected = match self.health_focus {
                    HealthFocus::Findings => &mut self.health_findings_selected,
                    HealthFocus::Configs => &mut self.health_configs_selected,
                };
                if len > 0 {
                    let previous = *selected;
                    *selected = (*selected as i32 + delta).rem_euclid(len as i32) as usize;
                    return *selected != previous;
                }
            }
            Tab::Upgrades => {
                let len = self.filtered_registry().len() as i32;
                if len > 0 {
                    let previous = self.upgrades_selected;
                    self.upgrades_selected =
                        (self.upgrades_selected as i32 + delta).rem_euclid(len) as usize;
                    return self.upgrades_selected != previous;
                }
            }
            Tab::Docs => {
                let len = self.filtered_docs().len() as i32;
                if len > 0 {
                    let previous = self.docs_selected;
                    self.docs_selected =
                        (self.docs_selected as i32 + delta).rem_euclid(len) as usize;
                    if self.docs_selected != previous {
                        self.docs_scroll = 0;
                    }
                    return self.docs_selected != previous;
                }
            }
            Tab::Harness => {
                let len = self.filtered_harness().len() as i32;
                if len > 0 {
                    let previous = self.harness_selected;
                    self.harness_selected =
                        (self.harness_selected as i32 + delta).rem_euclid(len) as usize;
                    if self.harness_selected != previous {
                        self.harness_scroll = 0;
                    }
                    return self.harness_selected != previous;
                }
            }
        }
        false
    }

    /// Scrolls the reading pane owned by the active tab. Docs and Harness both
    /// have one; every other tab ignores paging.
    fn scroll_pane(&mut self, delta: i32) -> bool {
        let field = match self.tab {
            Tab::Docs => &mut self.docs_scroll,
            Tab::Harness => &mut self.harness_scroll,
            _ => return false,
        };
        let previous = *field;
        *field = if delta >= 0 {
            field.saturating_add(delta as u16)
        } else {
            field.saturating_sub(delta.unsigned_abs() as u16)
        };
        *field != previous
    }

    fn handle_picker_key(&mut self, code: KeyCode) -> bool {
        let targets = self.editor_targets();
        match code {
            KeyCode::Esc | KeyCode::Char('e') => {
                self.picker_open = false;
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !targets.is_empty() {
                    self.picker_selected = (self.picker_selected + 1) % targets.len();
                    true
                } else {
                    false
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !targets.is_empty() {
                    self.picker_selected =
                        (self.picker_selected + targets.len() - 1) % targets.len();
                    true
                } else {
                    false
                }
            }
            KeyCode::Enter => {
                self.picker_open = false;
                if let Some(target) = targets.get(self.picker_selected) {
                    self.status = format!("opening {}...", target.label);
                    self.pending_editor = Some(target.path.clone());
                }
                true
            }
            _ => false,
        }
    }

    /// Opens a confirmation modal for the scope the current selection
    /// belongs to, naming the exact step count so "install 12 items into
    /// /repo/.claude" is a sentence someone can disagree with — "install?"
    /// is not. Silently does nothing if the harness state hasn't loaded yet,
    /// there is nothing selected, or the scope has nothing to do.
    fn open_harness_modal(&mut self, action: HarnessModalAction) -> bool {
        if !self.harness_ready {
            return false;
        }
        let rows = self.filtered_harness();
        if rows.is_empty() {
            return false;
        }
        let index = self.harness_selected.min(rows.len() - 1);
        let scope = rows[index].0;
        self.harness_plan_generation = self.harness_plan_generation.wrapping_add(1);
        let generation = self.harness_plan_generation;
        self.harness_modal = Some(HarnessModal {
            scope,
            action,
            steps: None,
            generation,
        });
        self.status = format!(
            "calculating harness {} ({} scope)...",
            action.label(),
            scope.label()
        );
        let tx = self.tx.clone();
        let root = self.harness_root(scope);
        tokio::task::spawn_blocking(move || {
            let steps = compute_harness_step_count(root, scope, action);
            let _ = tx.send(AppEvent::HarnessPlan {
                generation,
                scope,
                action,
                steps,
            });
        });
        true
    }

    fn harness_root(&self, scope: ReceiptScope) -> PathBuf {
        match scope {
            ReceiptScope::Project => self.harness_project_root.clone(),
            ReceiptScope::Global => self.home_dir.clone(),
        }
    }

    fn handle_harness_modal_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.harness_modal = None;
                true
            }
            KeyCode::Enter => {
                if self
                    .harness_modal
                    .is_some_and(|modal| modal.steps.is_some())
                    && let Some(modal) = self.harness_modal.take()
                {
                    self.run_harness_action(modal);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Runs the confirmed mutation off the async runtime's worker threads,
    /// through the exact same `Plan`/`Runner`/`record` path the CLI uses —
    /// two ways to mutate would be two places for the blocked-on-drift rule
    /// to be wrong, and this tab has no `--dry-run` to fall back on
    /// (`CON-002`, `SEC-001`). Refreshes the changed scope on completion
    /// rather than trusting its own plan, the same reason `pinst config
    /// status` re-reads the filesystem instead of believing a just-applied
    /// plan.
    fn run_harness_action(&mut self, modal: HarnessModal) {
        self.harness_busy = true;
        self.status = format!(
            "{}ing harness ({} scope)...",
            modal.action.label(),
            modal.scope.label()
        );

        let tx = self.tx.clone();
        let root = self.harness_root(modal.scope);
        let scope = modal.scope;
        let action = modal.action;

        tokio::task::spawn_blocking(move || {
            let source = harness_asset::resolve_source();
            let runner = crate::core::exec::Runner::new(false);
            let approve = |_: &crate::core::plan::Step| true;
            let auth = crate::core::engine::Authorizer { approve: &approve };

            match action {
                HarnessModalAction::Install => {
                    let mut plan = crate::core::plan::Plan::default();
                    if matches!(scope, ReceiptScope::Project)
                        && let Ok(scaffold) =
                            crate::core::harness::install::corpus_init::build_scaffold_plan(&root)
                    {
                        for step in scaffold.steps {
                            plan.push(step);
                        }
                    }
                    let options = harness_plan::InstallOptions {
                        vendor: Vendor::Claude,
                        scope,
                        root: root.clone(),
                        style: None,
                        force: false,
                        selection: harness_plan::Selection::All,
                    };
                    if let Ok(asset_plan) = harness_plan::build_install_plan(&source, &options) {
                        for step in asset_plan.steps {
                            plan.push(step);
                        }
                    }
                    let (reports, _summary) =
                        crate::core::engine::execute(&plan, &runner, &auth, &mut |_| {});
                    let _ = record::record_install(scope, Vendor::Claude, &root, &source, &reports);
                }
                HarnessModalAction::Uninstall => {
                    if let Ok(receipt_path) = Receipt::path_for(scope, &root)
                        && let Ok(Some(receipt)) = Receipt::load(&receipt_path)
                        && let Ok(plan) = harness_plan::build_uninstall_plan(
                            &source,
                            &receipt.entries,
                            &harness_plan::Selection::All,
                            false,
                        )
                    {
                        let (reports, _summary) =
                            crate::core::engine::execute(&plan, &runner, &auth, &mut |_| {});
                        let _ = record::record_uninstall(
                            scope,
                            Vendor::Claude,
                            &root,
                            receipt,
                            &reports,
                            false,
                        );
                    }
                }
            }

            let rows = harness_state::status(&source, Vendor::Claude, &root).unwrap_or_default();
            let _ = tx.send(AppEvent::HarnessScope { scope, rows });
        });
    }
}

fn compute_harness_step_count(
    root: PathBuf,
    scope: ReceiptScope,
    action: HarnessModalAction,
) -> Option<usize> {
    let source = harness_asset::resolve_source();
    let steps = match action {
        HarnessModalAction::Install => {
            let scaffold_steps = if matches!(scope, ReceiptScope::Project) {
                crate::core::harness::install::corpus_init::build_scaffold_plan(&root)
                    .ok()?
                    .pending_count()
            } else {
                0
            };
            let options = harness_plan::InstallOptions {
                vendor: Vendor::Claude,
                scope,
                root,
                style: None,
                force: false,
                selection: harness_plan::Selection::All,
            };
            harness_plan::build_install_plan(&source, &options)
                .ok()?
                .pending_count()
                + scaffold_steps
        }
        HarnessModalAction::Uninstall => {
            let receipt_path = Receipt::path_for(scope, &root).ok()?;
            let receipt = Receipt::load(&receipt_path).ok()??;
            harness_plan::build_uninstall_plan(
                &source,
                &receipt.entries,
                &harness_plan::Selection::All,
                false,
            )
            .ok()?
            .pending_count()
        }
    };
    (steps > 0).then_some(steps)
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

    fn rendered(app: &App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn event_outcomes_only_redraw_for_visible_changes() {
        let (_dir, mut app) = app(&[]);

        let ignored = app.handle_event(AppEvent::Term(Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        ))));
        assert_eq!(ignored, EventOutcome::default());

        let resized = app.handle_event(AppEvent::Term(Event::Resize(100, 30)));
        assert!(resized.redraw);
        assert!(!resized.quit);

        let probe = app.handle_event(AppEvent::Probe(ProbeResult {
            tool: app.registry[0].name.clone(),
            installed: true,
            version: Some("1.0.0".to_string()),
            path: None,
        }));
        assert!(probe.redraw);

        let quit = app.handle_event(AppEvent::Term(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        ))));
        assert!(quit.quit);
        assert!(!quit.redraw, "quit must not request a final frame");
    }

    #[test]
    fn opening_an_editor_is_an_explicit_event_outcome() {
        let (_dir, mut app) = app(&[]);
        press(&mut app, KeyCode::Char('e'));
        let outcome = app.handle_event(AppEvent::Term(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))));

        assert!(outcome.redraw);
        assert!(outcome.launch_editor);
        assert!(!outcome.quit);
    }

    #[test]
    fn stale_health_results_cannot_overwrite_a_newer_refresh() {
        let (_dir, mut app) = app(&[]);
        app.health_generation = 2;
        app.findings = vec![Finding {
            id: "new".to_string(),
            severity: crate::core::doctor::Severity::Info,
            message: "new diagnosis".to_string(),
            remediation: String::new(),
            fixable: false,
        }];

        let stale = app.handle_event(AppEvent::Health {
            generation: 1,
            findings: Vec::new(),
            configs: Vec::new(),
        });
        assert!(!stale.redraw);
        assert_eq!(app.findings[0].id, "new");

        let current = app.handle_event(AppEvent::Health {
            generation: 2,
            findings: Vec::new(),
            configs: Vec::new(),
        });
        assert!(current.redraw);
        assert!(app.findings.is_empty());
    }

    #[tokio::test]
    async fn closing_an_editor_starts_a_new_health_generation_and_reports_failures() {
        let (_dir, mut app) = app(&[]);
        let generation = app.health_generation;

        app.on_editor_closed(Err("stub failed".to_string()));

        assert_eq!(app.health_generation, generation + 1);
        assert!(app.status.contains("stub failed"), "{}", app.status);
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

        // Docs is no longer the last tab — Harness follows it — so cycling
        // forward once lands there, not back at the first tab.
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Harness);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.tab, Tab::Docs);

        // Cycling past the actual last tab (Harness) wraps to the first.
        press(&mut app, KeyCode::Char('5'));
        assert_eq!(app.tab, Tab::Harness);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Overview);
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

        let tools = app.filtered_docs();
        let (tool, page) = app.selected_doc_in(&tools).unwrap();
        assert_eq!(tool.name, "ripgrep");
        assert_eq!(page.unwrap().recipes.len(), 1);
    }

    #[test]
    fn a_tool_with_no_page_still_selects_and_reports_the_gap() {
        let (_dir, mut app) = app(&[]);
        press(&mut app, KeyCode::Char('4'));
        let tools = app.filtered_docs();
        let (tool, page) = app.selected_doc_in(&tools).unwrap();
        assert!(!tool.name.is_empty());
        assert!(page.is_none());
    }

    #[test]
    fn a_search_that_matches_nothing_leaves_the_reading_pane_empty() {
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        press(&mut app, KeyCode::Char('4'));
        typed(&mut app, "kubernetes");
        assert!(app.filtered_docs().is_empty());
        let tools = app.filtered_docs();
        assert!(app.selected_doc_in(&tools).is_none());
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
    fn health_and_upgrade_navigation_own_persistent_selections() {
        let (_dir, mut app) = app(&[]);
        app.health_ready = true;
        app.findings = (0..3)
            .map(|index| Finding {
                id: format!("finding.{index}"),
                severity: crate::core::doctor::Severity::Info,
                message: format!("finding {index}"),
                remediation: String::new(),
                fixable: false,
            })
            .collect();
        app.configs = (0..3)
            .map(|index| FileStatus {
                package: "test".to_string(),
                path: format!("config-{index}"),
                target: PathBuf::from(format!("/tmp/config-{index}")),
                state: crate::core::configs::FileState::Missing,
                templated: false,
            })
            .collect();

        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.health_findings_selected, 1);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.health_focus, HealthFocus::Configs);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.health_configs_selected, 1);

        press(&mut app, KeyCode::Char('3'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.upgrades_selected, 1);
        assert_eq!(app.health_findings_selected, 1);
    }

    #[test]
    fn changing_docs_selection_or_query_resets_page_scroll() {
        let (_dir, mut app) = app(&[("ripgrep", RIPGREP)]);
        app.tab = Tab::Docs;
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.docs_scroll, 5);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.docs_scroll, 0);
        press(&mut app, KeyCode::PageDown);
        typed(&mut app, "grep");
        assert_eq!(app.docs_scroll, 0);
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

    #[test]
    fn completed_upgrade_rows_never_remain_checking() {
        let (_dir, mut app) = app(&[]);
        app.tab = Tab::Upgrades;
        app.upgrades_ever_run = true;
        app.upgrades_loading = false;
        let name = app.registry[0].name.clone();
        app.upgrades.insert(
            name.clone(),
            UpgradeCheck {
                tool: name,
                result: None,
                state: crate::core::upgrade::UpgradeCheckState::Unsupported,
            },
        );

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

        assert!(rendered.contains("unsupported"), "{rendered}");
        assert!(
            !rendered.contains("checking..."),
            "a completed run must not retain an in-progress label"
        );
    }

    #[test]
    fn a_short_terminal_scrolls_tables_to_the_selected_final_row() {
        let (_dir, mut app) = app(&[]);
        app.tab = Tab::Overview;
        press(&mut app, KeyCode::Up);
        let final_tool = app.registry.last().unwrap().name.clone();

        let backend = ratatui::backend::TestBackend::new(100, 15);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(rendered.contains(&final_tool), "{rendered}");
    }

    #[test]
    fn health_upgrades_and_harness_reveal_their_final_rows() {
        let (_dir, mut app) = app(&[]);
        app.health_ready = true;
        app.findings = (0..20)
            .map(|index| Finding {
                id: format!("finding.{index}"),
                severity: crate::core::doctor::Severity::Info,
                message: format!("finding-{index}"),
                remediation: String::new(),
                fixable: false,
            })
            .collect();
        app.configs = (0..20)
            .map(|index| FileStatus {
                package: "test".to_string(),
                path: format!("config-{index}"),
                target: PathBuf::from(format!("/tmp/config-{index}")),
                state: crate::core::configs::FileState::Missing,
                templated: false,
            })
            .collect();
        app.tab = Tab::Health;
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Up);
        let health = rendered(&app, 100, 15);
        assert!(health.contains("finding-19"), "{health}");
        assert!(health.contains("config-19"), "{health}");

        app.tab = Tab::Upgrades;
        app.upgrades_ever_run = true;
        press(&mut app, KeyCode::Up);
        let final_tool = app.registry.last().unwrap().name.clone();
        let upgrades = rendered(&app, 100, 15);
        assert!(upgrades.contains(&final_tool), "{upgrades}");

        app.tab = Tab::Harness;
        app.harness_ready = true;
        app.harness_project = (0..20)
            .map(|index| seeded_row(&format!("asset-{index}"), HarnessState::Missing))
            .collect();
        press(&mut app, KeyCode::Up);
        let harness = rendered(&app, 100, 15);
        assert!(harness.contains("asset-19"), "{harness}");
    }

    #[test]
    fn a_long_docs_page_scrolls_to_its_final_line() {
        let long_page = format!(
            "what = \"Long page.\"\nstatus = \"draft\"\ngotchas = \"\"\"{}\"\"\"\n",
            (0..20)
                .map(|index| format!("line-{index}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let (_dir, mut app) = app(&[("ripgrep", &long_page)]);
        app.tab = Tab::Docs;
        typed(&mut app, "grep");
        for _ in 0..4 {
            press(&mut app, KeyCode::PageDown);
        }

        let backend = ratatui::backend::TestBackend::new(100, 15);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(rendered.contains("line-19"), "{rendered}");
    }

    use harness_state::AssetState as HarnessState;

    fn seeded_row(name: &str, state: HarnessState) -> AssetStatus {
        AssetStatus {
            kind: harness_state::AssetKindLabel::Skill,
            name: name.to_string(),
            target: PathBuf::from(format!("/tmp/{name}")),
            state,
        }
    }

    #[test]
    fn the_harness_tab_is_reachable_and_renders_both_scopes() {
        let (_dir, mut app) = app(&[]);
        app.harness_ready = true;
        app.harness_project = vec![seeded_row("demo", HarnessState::Linked)];
        app.harness_global = Vec::new();

        press(&mut app, KeyCode::Char('5'));
        assert_eq!(app.tab, Tab::Harness);

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
        assert!(rendered.contains("project"), "{rendered}");
        assert!(rendered.contains("global"), "{rendered}");
        assert!(rendered.contains("demo"), "{rendered}");
    }

    #[test]
    fn harness_scope_stat_counts_each_state_and_reports_the_root() {
        let (_dir, mut app) = app(&[]);
        app.harness_project_root = PathBuf::from("/repo/project");
        app.harness_project = vec![
            seeded_row("linked", HarnessState::Linked),
            seeded_row("copied", HarnessState::Copied),
            seeded_row("missing", HarnessState::Missing),
            seeded_row("drifted", HarnessState::Drifted),
            seeded_row("foreign", HarnessState::Foreign),
            seeded_row("unmanaged", HarnessState::Unmanaged),
        ];

        let stat = app.harness_scope_stat(ReceiptScope::Project);
        assert_eq!(stat.root, PathBuf::from("/repo/project"));
        assert_eq!(stat.total, 6);
        assert_eq!(stat.installed, 2);
        assert_eq!(stat.missing, 1);
        assert_eq!(stat.drifted, 2);
        assert_eq!(stat.unmanaged, 1);
    }

    fn scoped_row(
        name: &str,
        kind: harness_state::AssetKindLabel,
        target: &str,
        state: HarnessState,
    ) -> AssetStatus {
        AssetStatus {
            kind,
            name: name.to_string(),
            target: PathBuf::from(target),
            state,
        }
    }

    #[test]
    fn the_harness_tab_groups_scopes_with_roots_counts_and_target_paths() {
        let (_dir, mut app) = app(&[]);
        app.harness_ready = true;
        app.harness_project_root = PathBuf::from("/repo-root");
        app.home_dir = PathBuf::from("/home/tester");
        app.harness_project = vec![scoped_row(
            "demo",
            harness_state::AssetKindLabel::Skill,
            "/repo-root/.claude/skills/demo",
            HarnessState::Linked,
        )];
        app.harness_global = vec![scoped_row(
            "global-cmd",
            harness_state::AssetKindLabel::Command,
            "/home/tester/.claude/commands/global-cmd.md",
            HarnessState::Missing,
        )];
        app.tab = Tab::Harness;

        let rendered = rendered(&app, 140, 40);
        assert!(rendered.contains("project"), "{rendered}");
        assert!(rendered.contains("global"), "{rendered}");
        assert!(
            rendered.contains("/repo-root"),
            "project install root must show: {rendered}"
        );
        assert!(
            rendered.contains("/home/tester"),
            "global install root must show: {rendered}"
        );
        assert!(
            rendered.contains("/repo-root/.claude/skills/demo"),
            "the install path must show: {rendered}"
        );
        assert!(
            rendered.contains("1/1 installed") && rendered.contains("0/1 installed"),
            "per-scope counts must show: {rendered}"
        );
    }

    #[test]
    fn harness_search_filters_without_moving_other_tabs_cursors() {
        let (_dir, mut app) = app(&[]);
        app.harness_ready = true;
        app.harness_project = vec![
            seeded_row("alpha", HarnessState::Missing),
            seeded_row("beta", HarnessState::Missing),
        ];

        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.harness_selected, 1);
        assert_eq!(
            app.overview_selected, 0,
            "the Overview cursor must not move"
        );

        typed(&mut app, "alpha");
        assert_eq!(app.filtered_harness().len(), 1);
        assert_eq!(app.harness_selected, 0, "a new query resets the cursor");
    }

    #[tokio::test]
    async fn i_opens_a_modal_then_reports_the_step_count_and_esc_cancels_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let manifest = manifest::load(None).unwrap().manifest;
        let mut app = App::new(tx, manifest, PathBuf::from("/home/test"), catalogue);
        // Redirect the project scope at an empty sandbox rather than this
        // checkout — the modal's step count is computed by really building
        // an install plan, and this keeps that read entirely inside a
        // tempdir even though nothing is executed in this test.
        app.harness_project_root = tmp.path().to_path_buf();
        app.harness_ready = true;
        app.harness_project = vec![seeded_row("pinst", HarnessState::Missing)];
        app.harness_global = Vec::new();
        app.tab = Tab::Harness;

        press(&mut app, KeyCode::Char('i'));
        let calculating = app.harness_modal.expect("a modal should have opened");
        assert_eq!(calculating.steps, None);
        let event = rx.recv().await.expect("the plan count must report back");
        app.handle_event(event);
        let modal = app.harness_modal.expect("the modal should now be ready");
        assert_eq!(modal.action, HarnessModalAction::Install);
        assert!(
            modal.steps.is_some_and(|steps| steps > 0),
            "a fresh sandbox has something to install"
        );

        press(&mut app, KeyCode::Esc);
        assert!(app.harness_modal.is_none());
        assert!(
            std::fs::read_dir(tmp.path()).unwrap().next().is_none(),
            "Esc must not have written anything"
        );
    }

    #[tokio::test]
    async fn enter_on_the_install_modal_actually_installs_and_the_next_refresh_shows_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let manifest = manifest::load(None).unwrap().manifest;
        let mut app = App::new(tx, manifest, PathBuf::from("/home/test"), catalogue);

        app.harness_project_root = tmp.path().to_path_buf();
        app.harness_ready = true;
        app.harness_project = vec![seeded_row("pinst", HarnessState::Missing)];
        app.harness_global = Vec::new();
        app.tab = Tab::Harness;

        press(&mut app, KeyCode::Char('i'));
        assert!(app.harness_modal.is_some());
        let event = rx.recv().await.expect("the plan count must report back");
        app.handle_event(event);
        press(&mut app, KeyCode::Enter);
        assert!(app.harness_modal.is_none(), "confirming closes the modal");
        assert!(app.harness_busy);

        let event = rx.recv().await.expect("the spawned run must report back");
        app.handle_event(event);

        assert!(!app.harness_busy);
        assert!(
            app.harness_project.iter().any(|r| r.state.satisfied()),
            "{:?}",
            app.harness_project
        );
        assert!(
            tmp.path()
                .join(".claude/skills")
                .read_dir()
                .unwrap()
                .next()
                .is_some()
        );
    }

    #[tokio::test]
    async fn enter_on_the_uninstall_modal_removes_only_the_receipts_entries() {
        use crate::core::harness::asset::{self as raw_asset};
        use crate::core::harness::install::receipt::Receipt;

        let tmp = tempfile::tempdir().unwrap();
        // Install for real first (through the shared core path, not the
        // TUI), then plant an extra hand-made skill the receipt never
        // recorded — the row that must survive the uninstall untouched.
        let source = raw_asset::resolve_source();
        let opts = harness_plan::InstallOptions {
            vendor: Vendor::Claude,
            scope: ReceiptScope::Project,
            root: tmp.path().to_path_buf(),
            style: None,
            force: false,
            selection: harness_plan::Selection::Named {
                skills: vec!["pinst".to_string()],
                commands: vec![],
            },
        };
        let plan = harness_plan::build_install_plan(&source, &opts).unwrap();
        let runner = crate::core::exec::Runner::new(false);
        let approve = |_: &crate::core::plan::Step| true;
        let auth = crate::core::engine::Authorizer { approve: &approve };
        let (reports, _) = crate::core::engine::execute(&plan, &runner, &auth, &mut |_| {});
        record::record_install(
            ReceiptScope::Project,
            Vendor::Claude,
            tmp.path(),
            &source,
            &reports,
        )
        .unwrap();

        let skills_dir = Vendor::Claude.skills_dir(tmp.path());
        std::fs::create_dir_all(skills_dir.join("someone-elses-skill")).unwrap();
        std::fs::write(skills_dir.join("someone-elses-skill/SKILL.md"), "mine").unwrap();

        let receipt_path = Receipt::path_for(ReceiptScope::Project, tmp.path()).unwrap();
        let receipt = Receipt::load(&receipt_path).unwrap().unwrap();
        assert_eq!(receipt.entries.len(), 1, "sanity: only pinst was recorded");

        let dir = tempfile::tempdir().unwrap();
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let manifest = manifest::load(None).unwrap().manifest;
        let mut app = App::new(tx, manifest, PathBuf::from("/home/test"), catalogue);
        app.harness_project_root = tmp.path().to_path_buf();
        app.harness_ready = true;
        app.harness_project = harness_state::status(&source, Vendor::Claude, tmp.path()).unwrap();
        app.harness_global = Vec::new();
        app.tab = Tab::Harness;

        press(&mut app, KeyCode::Char('u'));
        let event = rx.recv().await.expect("the plan count must report back");
        app.handle_event(event);
        let modal = app
            .harness_modal
            .expect("uninstall should have something to do");
        assert_eq!(modal.action, HarnessModalAction::Uninstall);
        assert_eq!(modal.steps, Some(1));

        press(&mut app, KeyCode::Enter);
        let event = rx.recv().await.unwrap();
        app.handle_event(event);

        assert!(
            !Receipt::path_for(ReceiptScope::Project, tmp.path())
                .unwrap()
                .exists()
        );
        assert!(
            !skills_dir.join("pinst").exists(),
            "the receipt's own entry must be gone"
        );
        assert!(
            skills_dir.join("someone-elses-skill").exists(),
            "an entry the receipt never named must survive"
        );
        assert!(
            app.harness_project
                .iter()
                .any(|r| r.name == "someone-elses-skill"
                    && r.state == harness_state::AssetState::Unmanaged),
            "{:?}",
            app.harness_project
        );
    }

    #[test]
    fn a_harness_mutation_refreshes_only_its_changed_scope() {
        let (_dir, mut app) = app(&[]);
        app.harness_project = vec![seeded_row("old-project", HarnessState::Missing)];
        app.harness_global = vec![seeded_row("unchanged-global", HarnessState::Missing)];

        app.handle_event(AppEvent::HarnessScope {
            scope: ReceiptScope::Project,
            rows: vec![seeded_row("new-project", HarnessState::Linked)],
        });

        assert_eq!(app.harness_project[0].name, "new-project");
        assert_eq!(app.harness_global[0].name, "unchanged-global");
    }
}
