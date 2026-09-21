//! Turning a finished install/uninstall run into an updated receipt.
//!
//! Shared by `cli::commands::harness` and the TUI's Harness tab so neither
//! develops its own idea of what counts as "installed" after a run — the
//! exact drift this whole plan exists to close, and it would be an odd
//! place to reopen it by hand-rolling the receipt update twice.

use std::path::Path;

use color_eyre::eyre::Result;

use super::super::asset::{Asset, Source};
use super::super::vendor::Vendor;
use super::plan;
use super::receipt::{Entry, LinkStyle, Receipt, ReceiptScope};
use super::state;
use crate::core::plan::{Outcome, StepReport};

/// Only a real, failure-free run earns a receipt update: one recorded before
/// (or despite) failure would describe an intention rather than what is on
/// disk, and a later uninstall would then try to remove paths that were
/// never created.
pub fn record_install(
    scope: ReceiptScope,
    vendor: Vendor,
    root: &Path,
    source: &Source,
    reports: &[StepReport],
) -> Result<()> {
    if reports.iter().any(|r| r.outcome == Outcome::Failed) {
        return Ok(());
    }

    let resolved_style = match (source, scope) {
        (Source::Tree(_), ReceiptScope::Project) => LinkStyle::Symlink,
        _ => LinkStyle::Copy,
    };

    // Each report's step id already carries the asset's kind and name
    // (`plan::step_id`/`parse_step_id`), so the receipt is built straight
    // from what actually happened rather than re-deriving it from whatever
    // selection was asked for.
    let new_entries: Vec<Entry> = reports
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Ran | Outcome::Skipped))
        .filter_map(|r| plan::parse_step_id(&r.id))
        .map(|(kind, name)| {
            let target = state::target_path(
                vendor,
                root,
                &Asset {
                    kind,
                    name: name.clone(),
                    files: Vec::new(),
                },
            );
            Entry {
                kind: kind.into(),
                name,
                path: target,
                style: resolved_style,
            }
        })
        .collect();
    if new_entries.is_empty() {
        return Ok(());
    }

    let receipt_path = Receipt::path_for(scope, root)?;
    // Merge with whatever the receipt already recorded — a repeat install
    // with a narrower selection must not forget entries an earlier, broader
    // run put there.
    let mut merged = Receipt::load(&receipt_path)?
        .map(|r| r.entries)
        .unwrap_or_default();
    for entry in new_entries {
        merged.retain(|e| !(e.kind == entry.kind && e.name == entry.name));
        merged.push(entry);
    }
    let source_label = match source {
        Source::Tree(path) => path.display().to_string(),
        Source::Embedded => "embedded".to_string(),
    };
    let receipt = Receipt::new(scope, vendor.label(), &source_label, merged);
    receipt.save(&receipt_path)
}

/// What happened to the receipt file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallOutcome {
    /// Pruned entries removed; the rest is still there.
    Updated,
    /// Nothing was left in it, so it was deleted.
    DeletedEmpty,
    /// `purge` asked for it to go regardless of what remained.
    Purged,
}

/// Prunes every entry a successful step removed, rewrites or deletes the
/// receipt, and cleans up a vendor directory this run happened to empty.
pub fn record_uninstall(
    scope: ReceiptScope,
    vendor: Vendor,
    root: &Path,
    mut receipt: Receipt,
    reports: &[StepReport],
    purge: bool,
) -> Result<UninstallOutcome> {
    for report in reports {
        if matches!(report.outcome, Outcome::Ran | Outcome::Skipped)
            && let Some((kind, name)) = plan::parse_step_id(&report.id)
        {
            receipt.remove_entry(kind.into(), &name);
        }
    }

    let receipt_path = Receipt::path_for(scope, root)?;
    let outcome = if receipt.is_empty() || purge {
        Receipt::delete(&receipt_path)?;
        if receipt.is_empty() {
            UninstallOutcome::DeletedEmpty
        } else {
            UninstallOutcome::Purged
        }
    } else {
        receipt.save(&receipt_path)?;
        UninstallOutcome::Updated
    };

    // `remove_dir` refuses on anything non-empty, so this is a no-op
    // whenever something else — someone's own skill, a partial selection —
    // is still there.
    let _ = std::fs::remove_dir(vendor.skills_dir(root));
    let _ = std::fs::remove_dir(vendor.commands_dir(root));

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness::asset::AssetKind;
    use crate::core::plan::StepKind;

    fn report(id: &str, outcome: Outcome) -> StepReport {
        StepReport {
            id: id.to_string(),
            tool: None,
            kind: StepKind::Config,
            description: id.to_string(),
            outcome,
            detail: None,
            commands: Vec::new(),
        }
    }

    #[test]
    fn record_install_writes_nothing_when_a_step_failed() {
        let dir = tempfile::tempdir().unwrap();
        let reports = vec![
            report("harness:skill:demo", Outcome::Ran),
            report("harness:skill:other", Outcome::Failed),
        ];
        record_install(
            ReceiptScope::Project,
            Vendor::Claude,
            dir.path(),
            &Source::Embedded,
            &reports,
        )
        .unwrap();
        assert!(
            Receipt::load(&Receipt::path_for(ReceiptScope::Project, dir.path()).unwrap())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn record_install_merges_with_an_existing_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let path = Receipt::path_for(ReceiptScope::Project, dir.path()).unwrap();
        Receipt::new(
            ReceiptScope::Project,
            "claude",
            "embedded",
            vec![Entry {
                kind: AssetKind::Skill.into(),
                name: "existing".to_string(),
                path: dir.path().join("existing"),
                style: LinkStyle::Copy,
            }],
        )
        .save(&path)
        .unwrap();

        let reports = vec![report("harness:skill:new-one", Outcome::Ran)];
        record_install(
            ReceiptScope::Project,
            Vendor::Claude,
            dir.path(),
            &Source::Embedded,
            &reports,
        )
        .unwrap();

        let receipt = Receipt::load(&path).unwrap().unwrap();
        assert_eq!(receipt.entries.len(), 2);
        assert!(receipt.entries.iter().any(|e| e.name == "existing"));
        assert!(receipt.entries.iter().any(|e| e.name == "new-one"));
    }

    #[test]
    fn record_uninstall_deletes_the_receipt_once_it_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let receipt = Receipt::new(
            ReceiptScope::Project,
            "claude",
            "embedded",
            vec![Entry {
                kind: AssetKind::Skill.into(),
                name: "demo".to_string(),
                path: dir.path().join("demo"),
                style: LinkStyle::Copy,
            }],
        );
        let path = Receipt::path_for(ReceiptScope::Project, dir.path()).unwrap();
        receipt.save(&path).unwrap();

        let reports = vec![report("harness:skill:demo", Outcome::Ran)];
        let outcome = record_uninstall(
            ReceiptScope::Project,
            Vendor::Claude,
            dir.path(),
            receipt,
            &reports,
            false,
        )
        .unwrap();
        assert_eq!(outcome, UninstallOutcome::DeletedEmpty);
        assert!(!path.exists());
    }

    #[test]
    fn record_uninstall_keeps_the_rest_after_a_partial_run() {
        let dir = tempfile::tempdir().unwrap();
        let receipt = Receipt::new(
            ReceiptScope::Project,
            "claude",
            "embedded",
            vec![
                Entry {
                    kind: AssetKind::Skill.into(),
                    name: "a".to_string(),
                    path: dir.path().join("a"),
                    style: LinkStyle::Copy,
                },
                Entry {
                    kind: AssetKind::Skill.into(),
                    name: "b".to_string(),
                    path: dir.path().join("b"),
                    style: LinkStyle::Copy,
                },
            ],
        );
        let path = Receipt::path_for(ReceiptScope::Project, dir.path()).unwrap();
        receipt.save(&path).unwrap();

        let reports = vec![report("harness:skill:a", Outcome::Ran)];
        let outcome = record_uninstall(
            ReceiptScope::Project,
            Vendor::Claude,
            dir.path(),
            receipt,
            &reports,
            false,
        )
        .unwrap();
        assert_eq!(outcome, UninstallOutcome::Updated);
        let remaining = Receipt::load(&path).unwrap().unwrap();
        assert_eq!(remaining.entries.len(), 1);
        assert_eq!(remaining.entries[0].name, "b");
    }
}
