//! The projection invariants the old bash wiring script used to own: every
//! skill's `SKILL.md` frontmatter is well-formed, and the project scope's
//! `.claude/skills`/`.claude/commands` agree with `.agents/`.
//!
//! Every finding id prefix here is a **literal** in the format string it is
//! built from, never assembled so the prefix itself is computed.
//! `lesson.unenforced` (`harness::check`) greps `src/` for the id a lesson's
//! `**Check:**` line names, and a prefix built from parts would compile,
//! pass every test, and break that grep in silence. For the same reason,
//! no id from this module is spelled out again in this comment or any
//! other prose in `src/` — a comment containing one would satisfy
//! `lesson.unenforced` by itself, which is exactly what happened once
//! already when this module's predecessor was moved (see this plan's
//! `learnings.md`). Refer to them by shape, not by name, the way
//! `harness::check` already does for its own ids.

use std::collections::BTreeSet;
use std::path::Path;

use crate::core::doctor::{Finding, Severity};

use super::super::asset::{self, Asset, AssetKind, Source};
use super::super::frontmatter;
use super::super::vendor::Vendor;
use super::state::{self, AssetState};

/// Every projection and skill-source finding, for the project scope at
/// `project_root`. Global scope is never checked here — `just qc` and
/// `pinst harness check` are about *this repo's own* corpus and wiring, and
/// a machine's global harness is a different question `harness status`
/// already answers.
///
/// Deliberately resolves its own source from `<project_root>/.agents`
/// rather than taking one from the caller or falling back to
/// `asset::resolve_source`'s ambient discovery: a corpus root with no
/// `.agents/` of its own has nothing to check the wiring of, and falling
/// back to whatever `.agents/` the *process* happens to be running near
/// would check an unrelated tree's wiring against this one's `.ash/` —
/// exactly the failure a `cargo test` fixture with only a synthetic `.ash/`
/// caught the first time this ran.
pub fn run(project_root: &Path) -> Vec<Finding> {
    let agents_dir = project_root.join(asset::SUBDIR);
    if !agents_dir.is_dir() {
        return Vec::new();
    }
    let source = Source::Tree(agents_dir);
    let source = &source;

    let mut findings = Vec::new();
    let Ok(assets) = asset::enumerate(source) else {
        return findings;
    };

    // Source validation only makes sense against a real checkout — an
    // embedded copy was already a `Tree` once, at build time, and a
    // provisioned machine has no `.agents/` on disk to point a remediation
    // at.
    if let Source::Tree(root) = source {
        for skill in assets.iter().filter(|a| a.kind == AssetKind::Skill) {
            findings.extend(check_skill_source(root, skill));
        }
    }

    findings.extend(check_wiring(source, &assets, project_root));
    findings
}

fn check_skill_source(tree_root: &Path, skill: &Asset) -> Vec<Finding> {
    let mut out = Vec::new();
    let dir = tree_root.join("skills").join(&skill.name);
    let file = dir.join("SKILL.md");

    let Ok(text) = std::fs::read_to_string(&file) else {
        out.push(error(
            format!("skill.no-manifest.{}", skill.name),
            format!("{} has no SKILL.md", dir.display()),
            "add SKILL.md with name/description frontmatter, or remove the directory",
        ));
        return out;
    };

    let fm = frontmatter::parse(&text).ok().flatten();
    match fm.as_ref().and_then(|f| f.scalar("name")) {
        None => out.push(error(
            format!("skill.no-name.{}", skill.name),
            format!("{} frontmatter has no 'name:'", file.display()),
            format!("add 'name: {}'", skill.name),
        )),
        Some(declared) if declared != skill.name => out.push(error(
            format!("skill.name-mismatch.{}", skill.name),
            format!(
                "{} declares name '{declared}' but lives in '{}/'",
                file.display(),
                skill.name
            ),
            "make them agree — the directory name is what gets invoked",
        )),
        Some(_) => {}
    }

    if fm.as_ref().and_then(|f| f.get("description")).is_none() {
        out.push(error(
            format!("skill.no-description.{}", skill.name),
            format!("{} frontmatter has no 'description:'", file.display()),
            "add one — it is the only thing a model sees when deciding to load the skill",
        ));
    }

    if let Some(unquoted) = unquoted_values(&text) {
        out.push(warning(
            format!("skill.unquoted-value.{}", skill.name),
            format!(
                "{} frontmatter value(s) contain a colon and are not quoted: {unquoted}",
                file.display()
            ),
            "wrap the value in single quotes, doubling any apostrophe inside it",
        ));
    }

    out
}

/// The same heuristic the old bash wiring script used, in Rust: a `key: value` line
/// whose value is not quoted and itself contains `": "` reads as valid YAML
/// only by accident — the first colon a real parser meets ends the key.
/// `harness::frontmatter` accepts a plain scalar with an embedded colon by
/// design (`key: the full ADR: Context, Decision` is legitimate content
/// here), so this is a hygiene warning, not something the strict parser
/// itself rejects.
fn unquoted_values(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    let mut bad = Vec::new();
    for line in lines {
        if line.trim_end() == "---" {
            break;
        }
        let Some(colon) = line.find(':') else {
            continue;
        };
        let key = &line[..colon];
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c == '-')
        {
            continue;
        }
        let value = line[colon + 1..].trim_start();
        if value.is_empty() {
            continue;
        }
        if matches!(value.chars().next(), Some('"') | Some('\'')) {
            continue;
        }
        if value.contains(": ") {
            bad.push(key.to_string());
        }
    }
    (!bad.is_empty()).then(|| bad.join(" "))
}

/// Every asset's wiring, plus anything in the vendor directories that names
/// no known asset — the two questions the old bash wiring script's `--check`
/// mode answered.
fn check_wiring(source: &Source, assets: &[Asset], project_root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut known_skills = BTreeSet::new();
    let mut known_commands = BTreeSet::new();

    for asset in assets {
        match asset.kind {
            AssetKind::Skill => {
                known_skills.insert(asset.name.clone());
            }
            AssetKind::Command => {
                known_commands.insert(asset.name.clone());
            }
        }
        for vendor in Vendor::ALL {
            let target = state::target_path(vendor, project_root, asset);
            let Ok(current) = state::classify(source, asset, &target) else {
                continue;
            };
            let dest = target.display();
            match current {
                AssetState::Linked => {}
                AssetState::Missing => findings.push(error(
                    format!("wire.missing.{}", asset.name),
                    format!("{} is not wired into {}", asset.name, target.display()),
                    format!(
                        "run: pinst harness install --scope project --{} {} --drift overwrite",
                        match asset.kind {
                            AssetKind::Skill => "skill",
                            AssetKind::Command => "command",
                        },
                        asset.name
                    ),
                )),
                AssetState::Foreign => findings.push(error(
                    format!("wire.broken.{}", asset.name),
                    format!("{dest} is a symlink that does not resolve to the harness source"),
                    "run: pinst harness install --scope project --force",
                )),
                AssetState::Copied | AssetState::Drifted => findings.push(error(
                    format!("wire.not-a-link.{}", asset.name),
                    format!("{dest} exists but is not a symlink"),
                    "run: pinst harness install --scope project --drift overwrite",
                )),
                AssetState::Unmanaged => {}
            }
        }
    }

    for vendor in Vendor::ALL {
        findings.extend(check_unmanaged(
            vendor.skills_dir(project_root),
            &known_skills,
            false,
        ));
        findings.extend(check_unmanaged(
            vendor.commands_dir(project_root),
            &known_commands,
            true,
        ));
    }

    findings
}

/// Entries sitting in a vendor directory that name no asset this harness
/// knows about — a dangling link to a skill that was renamed or removed
/// (`wire.orphan`), or a real file/directory someone put there by hand
/// (`wire.unmanaged`). Only the first is a leftover from *this* harness;
/// the second is left alone forever, by design (`SEC-001`).
fn check_unmanaged(
    dir: std::path::PathBuf,
    known: &BTreeSet<String>,
    is_commands: bool,
) -> Vec<Finding> {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = if is_commands {
            if path.extension().is_none_or(|e| e != "md") {
                continue;
            }
            path.file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string()
        } else {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string()
        };
        if known.contains(&name) {
            continue;
        }

        let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        if is_symlink {
            findings.push(warning(
                format!("wire.orphan.{name}"),
                format!("{} links to a skill that no longer exists", path.display()),
                "remove it, or run: pinst harness uninstall --scope project --all --force",
            ));
        } else {
            findings.push(warning(
                format!("wire.unmanaged.{name}"),
                format!("{} is not managed by .agents/", path.display()),
                "move it into .agents/skills/<name> (or commands/) so it is versioned with the \
                 rest of the harness",
            ));
        }
    }
    findings
}

fn error(
    id: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    Finding {
        id: id.into(),
        severity: Severity::Error,
        message: message.into(),
        remediation: remediation.into(),
        fixable: false,
    }
}

fn warning(
    id: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    Finding {
        id: id.into(),
        severity: Severity::Warning,
        message: message.into(),
        remediation: remediation.into(),
        fixable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything lives under one tempdir standing in for a project root —
    /// `.agents/` (the source `run` derives itself) and `.claude/` (what
    /// `run` checks it against) side by side, exactly as they sit in a real
    /// repo.
    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/skills/demo")).unwrap();
        std::fs::write(
            dir.path().join(".agents/skills/demo/SKILL.md"),
            "---\nname: demo\ndescription: a test skill\n---\nbody\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/commands")).unwrap();
        std::fs::write(dir.path().join(".agents/commands/demo.md"), "command body").unwrap();
        dir
    }

    #[test]
    fn a_project_with_no_dot_agents_has_nothing_to_check() {
        let dir = tempfile::tempdir().unwrap();
        assert!(run(dir.path()).is_empty());
    }

    #[test]
    fn a_healthy_source_and_a_fresh_wiring_report_nothing_but_missing() {
        let dir = project();
        let findings = run(dir.path());
        // Nothing is wired yet, so both assets are wire.missing and
        // nothing else — the skill source itself is well-formed.
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings.iter().all(|f| f.id.starts_with("wire.missing.")));
    }

    #[test]
    fn a_skill_with_no_skill_md_is_no_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/skills/ghost")).unwrap();
        // A non-.md file keeps the directory from being pruned as empty by
        // `asset::enumerate`.
        std::fs::write(dir.path().join(".agents/skills/ghost/notes.txt"), "x").unwrap();
        let findings = run(dir.path());
        assert!(
            findings.iter().any(|f| f.id == "skill.no-manifest.ghost"),
            "{findings:?}"
        );
    }

    #[test]
    fn a_name_mismatch_and_a_missing_description_are_both_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/skills/foo")).unwrap();
        std::fs::write(
            dir.path().join(".agents/skills/foo/SKILL.md"),
            "---\nname: bar\n---\n",
        )
        .unwrap();
        let findings = run(dir.path());
        assert!(findings.iter().any(|f| f.id == "skill.name-mismatch.foo"));
        assert!(findings.iter().any(|f| f.id == "skill.no-description.foo"));
    }

    #[test]
    fn an_unquoted_colon_space_value_is_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/skills/foo")).unwrap();
        std::fs::write(
            dir.path().join(".agents/skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: type(scope): subject\n---\n",
        )
        .unwrap();
        let findings = run(dir.path());
        let hit = findings
            .iter()
            .find(|f| f.id == "skill.unquoted-value.foo")
            .expect("expected an unquoted-value warning");
        assert_eq!(hit.severity, Severity::Warning);
    }

    #[test]
    fn a_quoted_value_with_a_colon_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/skills/foo")).unwrap();
        std::fs::write(
            dir.path().join(".agents/skills/foo/SKILL.md"),
            "---\nname: foo\ndescription: 'type(scope): subject'\n---\n",
        )
        .unwrap();
        let findings = run(dir.path());
        assert!(
            !findings
                .iter()
                .any(|f| f.id.starts_with("skill.unquoted-value"))
        );
    }

    #[test]
    fn a_correctly_linked_asset_reports_nothing() {
        let dir = project();
        let source = Source::Tree(dir.path().join(".agents"));
        let opts = super::super::plan::InstallOptions {
            vendor: Vendor::Claude,
            scope: super::super::receipt::ReceiptScope::Project,
            root: dir.path().to_path_buf(),
            style: None,
            force: false,
            drift: super::super::plan::DriftResolution::Overwrite,
            selection: super::super::plan::Selection::All,
        };
        let plan = super::super::plan::build_install_plan(&source, &opts).unwrap();
        let runner = crate::core::exec::Runner::new(false);
        let approve = |_: &crate::core::plan::Step| true;
        let auth = crate::core::engine::Authorizer { approve: &approve };
        crate::core::engine::execute(&plan, &runner, &auth, &mut |_| {});

        let findings = run(dir.path());
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_real_file_where_a_link_belongs_is_not_a_link() {
        let dir = project();
        let skills_dir = Vendor::Claude.skills_dir(dir.path());
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::create_dir_all(skills_dir.join("demo")).unwrap();
        std::fs::write(skills_dir.join("demo/SKILL.md"), "not linked").unwrap();

        let findings = run(dir.path());
        assert!(
            findings.iter().any(|f| f.id == "wire.not-a-link.demo"),
            "{findings:?}"
        );
    }

    #[test]
    fn a_dangling_symlink_is_broken() {
        let dir = project();
        let skills_dir = Vendor::Claude.skills_dir(dir.path());
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::os::unix::fs::symlink("/nonexistent", skills_dir.join("demo")).unwrap();

        let findings = run(dir.path());
        assert!(
            findings.iter().any(|f| f.id == "wire.broken.demo"),
            "{findings:?}"
        );
    }

    #[test]
    fn an_orphan_link_and_an_unmanaged_directory_are_told_apart() {
        let dir = project();
        let skills_dir = Vendor::Claude.skills_dir(dir.path());
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::os::unix::fs::symlink("/nonexistent", skills_dir.join("gone")).unwrap();
        std::fs::create_dir_all(skills_dir.join("someones-own")).unwrap();
        std::fs::write(skills_dir.join("someones-own/SKILL.md"), "mine").unwrap();

        let findings = run(dir.path());
        assert!(
            findings.iter().any(|f| f.id == "wire.orphan.gone"),
            "{findings:?}"
        );
        assert!(
            findings
                .iter()
                .any(|f| f.id == "wire.unmanaged.someones-own"),
            "{findings:?}"
        );
    }
}
