//! The record of what an install actually wrote — the input uninstall works
//! from, so it can remove exactly what it put down and nothing a person
//! wrote by hand (`SEC-001`).
//!
//! Two locations, one per scope, and the global one is deliberately **not**
//! `~/.ash/harness.json`: a bare `.ash/` at `$HOME` would be found by
//! `harness::root::CorpusRoot::discover` from any directory outside a real
//! repo, and `pinst harness check` would then confidently report a corpus
//! that does not exist as clean (`ALT-002`). The project receipt, by
//! contrast, belongs beside the corpus it describes: `<repo>/.ash/harness.json`.

use std::path::{Path, PathBuf};

use color_eyre::eyre::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::state::AssetKindLabel;
use crate::cli::ScopeArg;

/// How an asset was materialized — mirrors the two representations
/// `core::configs` already has for the same choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkStyle {
    Symlink,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptScope {
    Project,
    Global,
}

impl ReceiptScope {
    pub fn label(self) -> &'static str {
        match self {
            ReceiptScope::Project => "project",
            ReceiptScope::Global => "global",
        }
    }
}

impl TryFrom<ScopeArg> for ReceiptScope {
    type Error = color_eyre::eyre::Report;

    fn try_from(value: ScopeArg) -> Result<Self> {
        match value {
            ScopeArg::Project => Ok(ReceiptScope::Project),
            ScopeArg::Global => Ok(ReceiptScope::Global),
            ScopeArg::Both => {
                color_eyre::eyre::bail!("a receipt describes one scope; `both` is not one")
            }
        }
    }
}

/// One asset this install wrote, and exactly where — the unit uninstall
/// removes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub kind: AssetKindLabel,
    pub name: String,
    pub path: PathBuf,
    pub style: LinkStyle,
}

/// What one `pinst harness install` run at one scope left behind.
///
/// `#[serde(deny_unknown_fields)]`: this file is hand-editable the moment
/// someone debugs an install, and serde silently dropping a misspelled key
/// would load a receipt that means something other than what it says
/// (LESSON-017).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema_version: u32,
    pub scope: ReceiptScope,
    pub vendor: String,
    /// What `.agents/` was read from at install time — a tree's path, or
    /// `"embedded"`. Informational: uninstall works from `entries` alone.
    pub source: String,
    pub installed_at: String,
    pub pinst_version: String,
    pub entries: Vec<Entry>,
}

pub const RECEIPT_SCHEMA_VERSION: u32 = 1;

impl Receipt {
    pub fn new(scope: ReceiptScope, vendor: &str, source: &str, entries: Vec<Entry>) -> Self {
        Self {
            schema_version: RECEIPT_SCHEMA_VERSION,
            scope,
            vendor: vendor.to_string(),
            source: source.to_string(),
            installed_at: now_rfc3339(),
            pinst_version: env!("CARGO_PKG_VERSION").to_string(),
            entries,
        }
    }

    /// `<repo>/.ash/harness.json` for a project install,
    /// `${XDG_STATE_HOME:-~/.local/state}/pinst/harness.json` for a global
    /// one.
    pub fn path_for(scope: ReceiptScope, project_root: &Path) -> Result<PathBuf> {
        match scope {
            ReceiptScope::Project => Ok(project_root.join(".ash").join("harness.json")),
            ReceiptScope::Global => Ok(state_dir()?.join("pinst").join("harness.json")),
        }
    }

    pub fn load(path: &Path) -> Result<Option<Self>> {
        if !path.is_file() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let receipt: Receipt = serde_json::from_str(&text)
            .with_context(|| format!("parsing {} as a harness receipt", path.display()))?;
        Ok(Some(receipt))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }
}

/// `$XDG_STATE_HOME`, or `~/.local/state` — the standard fallback per the
/// XDG base directory spec, and deliberately not `~/.ash`, which is the
/// whole reason this function exists rather than reusing `home_dir`
/// directly (`ALT-002`).
fn state_dir() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("XDG_STATE_HOME") {
        let path = PathBuf::from(&value);
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }
    Ok(crate::core::home_dir()?.join(".local/state"))
}

fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    // Reuses `harness::id`'s date arithmetic rather than a second copy of
    // it — a `time`/`chrono` dependency for one timestamp field is not worth
    // adding to a binary tuned for size, and this repo already carries the
    // arithmetic pinned by `id`'s own tests.
    let (y, mo, d) = super::super::id::civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_and_global_receipts_never_share_a_path() {
        let root = Path::new("/repo");
        let project = Receipt::path_for(ReceiptScope::Project, root).unwrap();
        let global = Receipt::path_for(ReceiptScope::Global, root).unwrap();
        assert_eq!(project, Path::new("/repo/.ash/harness.json"));
        assert_ne!(project, global);
        // The important property: the global receipt is never under .ash/,
        // anywhere — ALT-002 is precisely the bug this asserts against.
        assert!(!global.starts_with("/repo"));
        assert!(!global.to_string_lossy().contains("/.ash"));
    }

    #[test]
    fn xdg_state_home_is_honoured_when_set() {
        let _guard = crate::core::source::test_env_lock();
        unsafe { std::env::set_var("XDG_STATE_HOME", "/custom/state") };
        let path = Receipt::path_for(ReceiptScope::Global, Path::new("/repo")).unwrap();
        unsafe { std::env::remove_var("XDG_STATE_HOME") };
        assert_eq!(path, Path::new("/custom/state/pinst/harness.json"));
    }

    #[test]
    fn a_receipt_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("harness.json");
        let receipt = Receipt::new(
            ReceiptScope::Project,
            "claude",
            "embedded",
            vec![Entry {
                kind: AssetKindLabel::Skill,
                name: "demo".to_string(),
                path: dir.path().join("skills/demo"),
                style: LinkStyle::Copy,
            }],
        );
        receipt.save(&path).unwrap();
        let loaded = Receipt::load(&path).unwrap().unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].name, "demo");
        assert_eq!(loaded.schema_version, RECEIPT_SCHEMA_VERSION);
    }

    #[test]
    fn a_missing_receipt_loads_as_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Receipt::load(&dir.path().join("nope.json")).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn an_unknown_field_is_rejected_rather_than_silently_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("harness.json");
        std::fs::write(
            &path,
            r#"{"schema_version":1,"scope":"project","vendor":"claude","source":"embedded","installed_at":"2026-01-01T00:00:00Z","pinst_version":"0.4.0","entries":[],"typo_field":true}"#,
        )
        .unwrap();
        assert!(Receipt::load(&path).is_err());
    }
}
