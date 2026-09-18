//! The machine-facing half of the CLI contract.
//!
//! Every command emits the same versioned envelope on **stdout** and puts all
//! human commentary on **stderr**, so `pinst <cmd> --json | jq` never has to
//! strip progress text out of the payload. Exit codes are part of the
//! contract too: an agent branches on them instead of parsing prose.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use color_eyre::eyre::Result;
use serde::Serialize;

/// Bumped only on a breaking change to the envelope shape.
pub const OUTPUT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Everything the command was asked to do succeeded.
    Ok,
    /// The command ran fine but found things the operator should act on
    /// (drift, missing tools, available upgrades). Maps to exit code 3.
    Issues,
    /// The command could not complete.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0,
    Failure = 1,
    Usage = 2,
    Issues = 3,
}

impl ExitCode {
    pub fn from_status(status: Status) -> Self {
        match status {
            Status::Ok => ExitCode::Success,
            Status::Issues => ExitCode::Issues,
            Status::Error => ExitCode::Failure,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    pub schema_version: u32,
    pub command: String,
    pub status: Status,
    /// True when nothing was actually mutated because `--dry-run` was set.
    pub dry_run: bool,
    pub items: Vec<T>,
    pub errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<serde_json::Value>,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(command: &str, status: Status, items: Vec<T>) -> Self {
        Self {
            schema_version: OUTPUT_SCHEMA_VERSION,
            command: command.to_string(),
            status,
            dry_run: false,
            items,
            errors: Vec::new(),
            summary: None,
        }
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn errors(mut self, errors: Vec<String>) -> Self {
        self.errors = errors;
        self
    }

    pub fn summary(mut self, summary: serde_json::Value) -> Self {
        self.summary = Some(summary);
        self
    }
}

/// Shared execution context: the global flags plus the derived interactivity
/// decision every command needs.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub json: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub quiet: bool,
    pub manifest_path: Option<PathBuf>,
}

impl Ctx {
    /// Whether pinst may stop and ask a human something.
    ///
    /// Never in JSON mode (an agent reading stdout would deadlock on a
    /// prompt) and never when stdout is not a terminal.
    pub fn interactive(&self) -> bool {
        !self.json && std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
    }

    /// Human commentary. Always stderr, suppressed by `--quiet`, and never
    /// emitted in JSON mode.
    pub fn note(&self, message: impl std::fmt::Display) {
        if self.quiet || self.json {
            return;
        }
        let _ = writeln!(std::io::stderr(), "{message}");
    }

    /// Emits the envelope as JSON on stdout and returns the matching exit
    /// code. Human mode prints nothing here — the caller renders its own
    /// table — so the two paths never double-print.
    pub fn finish<T: Serialize>(&self, envelope: Envelope<T>) -> Result<ExitCode> {
        if self.json {
            let mut stdout = std::io::stdout().lock();
            serde_json::to_writer_pretty(&mut stdout, &envelope)?;
            writeln!(stdout)?;
        }
        Ok(ExitCode::from_status(envelope.status))
    }

    /// Asks a yes/no question. `--yes` answers yes; a non-interactive session
    /// answers no, since a confirmation gate that silently self-approves is
    /// not a gate at all.
    pub fn confirm(&self, question: &str) -> bool {
        if self.yes {
            return true;
        }
        if !self.interactive() {
            return false;
        }
        let _ = write!(std::io::stderr(), "{question} [y/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Item {
        name: String,
    }

    fn ctx() -> Ctx {
        Ctx {
            json: true,
            dry_run: false,
            yes: false,
            quiet: false,
            manifest_path: None,
        }
    }

    #[test]
    fn envelope_serializes_with_the_documented_shape() {
        let envelope = Envelope::new(
            "list",
            Status::Ok,
            vec![Item {
                name: "zsh".to_string(),
            }],
        )
        .dry_run(true)
        .summary(serde_json::json!({"total": 1}));

        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["schema_version"], OUTPUT_SCHEMA_VERSION);
        assert_eq!(value["command"], "list");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["dry_run"], true);
        assert_eq!(value["items"][0]["name"], "zsh");
        assert_eq!(value["summary"]["total"], 1);
        assert!(value["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn status_maps_to_documented_exit_codes() {
        assert_eq!(ExitCode::from_status(Status::Ok), ExitCode::Success);
        assert_eq!(ExitCode::from_status(Status::Issues), ExitCode::Issues);
        assert_eq!(ExitCode::from_status(Status::Error), ExitCode::Failure);
        assert_eq!(ExitCode::Usage as i32, 2);
    }

    #[test]
    fn json_mode_is_never_interactive() {
        // An agent reading stdout must never be asked a question.
        assert!(!ctx().interactive());
        // ...and an unauthorized confirmation gate stays closed.
        assert!(!ctx().confirm("do the risky thing?"));
        assert!(Ctx { yes: true, ..ctx() }.confirm("do the risky thing?"));
    }
}
