//! The plan: an ordered, serializable description of everything a mutating
//! command would do.
//!
//! Every mutation in pinst is built as a `Plan` first and then either printed
//! (`--dry-run`) or executed. Nothing mutates outside this path, so what
//! `--dry-run` shows and what a real run does cannot drift apart.

use std::path::PathBuf;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Run with `sh -c`. `sudo` appears inside the command text itself, the
    /// same way the upstream install docs write it.
    Shell {
        command: String,
    },
    Download {
        url: String,
        dest: PathBuf,
    },
    /// Symlink `target` -> `source`.
    Link {
        source: PathBuf,
        target: PathBuf,
    },
    /// Write literal content to `target`. The bytes stay out of the
    /// serialized plan (a config file's contents are not plan metadata);
    /// only their length is reported.
    Write {
        target: PathBuf,
        bytes: usize,
        #[serde(skip)]
        #[schemars(skip)]
        content: Arc<Vec<u8>>,
    },
    /// Move an existing file aside before writing over its path.
    Backup {
        path: PathBuf,
        to: PathBuf,
    },
}

impl Action {
    pub fn describe(&self) -> String {
        match self {
            Action::Shell { command } => command.clone(),
            Action::Download { url, dest } => format!("download {url} -> {}", dest.display()),
            Action::Link { source, target } => {
                format!("link {} -> {}", target.display(), source.display())
            }
            Action::Write { target, bytes, .. } => {
                format!("write {} ({bytes} bytes)", target.display())
            }
            Action::Backup { path, to } => {
                format!("back up {} -> {}", path.display(), to.display())
            }
        }
    }

    pub fn privilege(&self) -> Privilege {
        match self {
            Action::Shell { command } => {
                if command.contains("sudo ") {
                    Privilege::Sudo
                } else if command.contains("curl ") && command.contains('|') {
                    Privilege::RemoteScript
                } else {
                    Privilege::User
                }
            }
            Action::Download { .. } => Privilege::Network,
            _ => Privilege::User,
        }
    }
}

/// What kind of authority an action needs. Surfaced in plans so the blast
/// radius of a run is visible before it happens, not after.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    User,
    Sudo,
    /// Pipes a downloaded script into a shell — unverifiable by construction.
    RemoteScript,
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    AptUpdate,
    Install,
    Upgrade,
    PostInstall,
    Config,
    /// No automated path exists; the step carries instructions instead.
    Manual,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "state", content = "reason")]
pub enum StepState {
    /// Will run.
    Pending,
    /// Already satisfied — the idempotency signal.
    Skipped(String),
    /// Cannot be automated; `reason` is the operator instruction.
    Blocked(String),
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Step {
    /// Stable identifier, e.g. `install:zsh` or `post:zsh:0`.
    pub id: String,
    pub kind: StepKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub description: String,
    pub actions: Vec<Action>,
    pub privilege: Privilege,
    /// Requires `--yes` or an interactive confirmation.
    pub confirm: bool,
    /// A `sh -c` check evaluated immediately before running: success means
    /// the step is already done and is skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    pub state: StepState,
}

impl Step {
    pub fn new(id: impl Into<String>, kind: StepKind, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            tool: None,
            description: description.into(),
            actions: Vec::new(),
            privilege: Privilege::User,
            confirm: false,
            condition: None,
            state: StepState::Pending,
        }
    }

    pub fn tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    pub fn actions(mut self, actions: Vec<Action>) -> Self {
        self.privilege = actions
            .iter()
            .map(Action::privilege)
            .max_by_key(|p| match p {
                Privilege::Sudo => 3,
                Privilege::RemoteScript => 2,
                Privilege::Network => 1,
                Privilege::User => 0,
            })
            .unwrap_or(Privilege::User);
        self.actions = actions;
        self
    }

    pub fn confirm(mut self, confirm: bool) -> Self {
        self.confirm = confirm;
        self
    }

    pub fn condition(mut self, condition: Option<String>) -> Self {
        self.condition = condition;
        self
    }

    pub fn skipped(mut self, reason: impl Into<String>) -> Self {
        self.state = StepState::Skipped(reason.into());
        self
    }

    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.state = StepState::Blocked(reason.into());
        self
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, StepState::Pending)
    }
}

#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct Plan {
    pub steps: Vec<Step>,
}

impl Plan {
    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn pending(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|s| s.is_pending())
    }

    pub fn pending_count(&self) -> usize {
        self.pending().count()
    }
}

/// What happened to one step during execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// `--dry-run`: this is what would have run.
    WouldRun,
    Ran,
    /// Already satisfied, or its condition said so at execution time.
    Skipped,
    /// Needed authorization that was not granted.
    NeedsConfirmation,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StepReport {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub kind: StepKind,
    pub description: String,
    pub outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub commands: Vec<String>,
}

impl StepReport {
    pub fn from_step(step: &Step, outcome: Outcome, detail: Option<String>) -> Self {
        Self {
            id: step.id.clone(),
            tool: step.tool.clone(),
            kind: step.kind,
            description: step.description.clone(),
            outcome,
            detail,
            commands: step.actions.iter().map(Action::describe).collect(),
        }
    }
}
