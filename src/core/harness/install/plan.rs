//! Turning a selection of harness assets into a `core::plan::Plan` — the
//! same `Link`/`Write`/`Backup`/`Remove` vocabulary every other mutating
//! command in pinst builds, so `--dry-run`, the authorization gate and the
//! JSON envelope come for free and cannot drift from what a real run does
//! (`CON-002`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use color_eyre::eyre::Result;

use super::super::asset::{self, Asset, AssetKind, Source};
use super::super::vendor::Vendor;
use super::receipt::{LinkStyle, ReceiptScope};
use super::state::{self, AssetState};
use crate::core::plan::{Action, Plan, Step, StepKind};

/// Which assets an install or uninstall acts on.
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Named {
        skills: Vec<String>,
        commands: Vec<String>,
    },
}

impl Selection {
    fn includes(&self, asset: &Asset) -> bool {
        match self {
            Selection::All => true,
            Selection::Named { skills, commands } => match asset.kind {
                AssetKind::Skill => skills.iter().any(|s| s == &asset.name),
                AssetKind::Command => commands.iter().any(|c| c == &asset.name),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub vendor: Vendor,
    pub scope: ReceiptScope,
    /// The project root, or `$HOME` for a global install.
    pub root: PathBuf,
    /// An explicit `--copy`/`--link`, overriding the scope-based default.
    pub style: Option<LinkStyle>,
    pub force: bool,
    pub selection: Selection,
}

/// Symlink when the source is a checkout *and* the scope is project;
/// copy otherwise. A link from `~/.claude` into a git worktree dangles the
/// moment that worktree is removed, and this repo is normally worked on
/// through disposable worktrees (`ALT-007`).
fn desired_style(
    source: &Source,
    scope: ReceiptScope,
    override_style: Option<LinkStyle>,
) -> LinkStyle {
    if let Some(style) = override_style {
        return style;
    }
    match (source, scope) {
        (Source::Tree(_), ReceiptScope::Project) => LinkStyle::Symlink,
        _ => LinkStyle::Copy,
    }
}

/// `harness:skill:<name>` / `harness:command:<name>` — stable across a
/// build and a status read, and parsed back by `parse_step_id` so the two
/// never have to agree by construction alone.
pub fn step_id(kind: AssetKind, name: &str) -> String {
    let kind = match kind {
        AssetKind::Skill => "skill",
        AssetKind::Command => "command",
    };
    format!("harness:{kind}:{name}")
}

pub fn parse_step_id(id: &str) -> Option<(AssetKind, String)> {
    let rest = id.strip_prefix("harness:")?;
    let (kind, name) = rest.split_once(':')?;
    let kind = match kind {
        "skill" => AssetKind::Skill,
        "command" => AssetKind::Command,
        _ => return None,
    };
    Some((kind, name.to_string()))
}

/// Builds the plan that installs every selected asset. Already-satisfied
/// assets are `Skipped`; a `Foreign` target (a symlink pointing somewhere
/// else) is `Blocked` unless `--force`; anything else — `Missing` or
/// `Drifted` — is backed up first when there is real content to lose, then
/// written or linked fresh, exactly as `ConfigSet::build_plan_where` treats
/// the same six states.
pub fn build_install_plan(source: &Source, options: &InstallOptions) -> Result<Plan> {
    let assets = asset::enumerate(source)?;
    let style = desired_style(source, options.scope, options.style);
    let stamp = timestamp();
    let mut plan = Plan::default();

    for asset in assets.iter().filter(|a| options.selection.includes(a)) {
        let target = state::target_path(options.vendor, &options.root, asset);
        let id = step_id(asset.kind, &asset.name);
        let kind_label = match asset.kind {
            AssetKind::Skill => "skill",
            AssetKind::Command => "command",
        };
        let description = format!("{kind_label} {} -> {}", asset.name, target.display());
        let current = state::classify(source, asset, &target)?;

        let satisfied = matches!(
            (current, style),
            (AssetState::Linked, LinkStyle::Symlink) | (AssetState::Copied, LinkStyle::Copy)
        );
        if satisfied {
            plan.push(
                Step::new(id, StepKind::Config, description)
                    .tool(&asset.name)
                    .skipped("already installed"),
            );
            continue;
        }

        if current == AssetState::Foreign && !options.force {
            plan.push(
                Step::new(id, StepKind::Config, description)
                    .tool(&asset.name)
                    .blocked(format!(
                        "{} is a symlink to something else; rerun with --force to replace it",
                        target.display()
                    )),
            );
            continue;
        }

        let mut actions = Vec::new();
        if matches!(current, AssetState::Drifted)
            || (current == AssetState::Foreign && options.force)
        {
            actions.push(Action::Backup {
                path: target.clone(),
                to: backup_path(&target, &stamp),
            });
        }

        match style {
            LinkStyle::Symlink => {
                let source_path = asset::source_path(source, &state::source_relative(asset))?;
                actions.push(Action::Link {
                    source: source_path,
                    target: target.clone(),
                });
            }
            // A command's target *is* its one file; a skill's target is the
            // directory its files land in. Joining an empty relative path
            // onto a file target (as `relative_within_asset` returns for a
            // command) still appends a path separator, which turned "write
            // this file" into "write into a directory of this name" and
            // failed with ENOTDIR/EISDIR — caught by hand-testing global
            // copy-mode installs before this shipped.
            LinkStyle::Copy if asset.kind == AssetKind::Command => {
                let content = asset::content(source, asset.entry())?;
                actions.push(Action::Write {
                    target: target.clone(),
                    bytes: content.len(),
                    content: Arc::new(content),
                });
            }
            LinkStyle::Copy => {
                for file in &asset.files {
                    let within = relative_within_asset(asset, file);
                    let content = asset::content(source, file)?;
                    actions.push(Action::Write {
                        target: target.join(within),
                        bytes: content.len(),
                        content: Arc::new(content),
                    });
                }
            }
        }

        plan.push(
            Step::new(id, StepKind::Config, description)
                .tool(&asset.name)
                .actions(actions),
        );
    }

    Ok(plan)
}

/// Where one of a skill's files lands under the skill's own target
/// directory — `SKILL.md` for `skills/demo/SKILL.md`. A command has exactly
/// one file, so this is always empty for it (the target *is* the file).
fn relative_within_asset(asset: &Asset, file: &Path) -> PathBuf {
    match asset.kind {
        AssetKind::Skill => {
            let root = Path::new("skills").join(&asset.name);
            file.strip_prefix(&root).unwrap_or(file).to_path_buf()
        }
        AssetKind::Command => PathBuf::new(),
    }
}

fn backup_path(target: &Path, stamp: &str) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(format!(".pre-pinst.{stamp}"));
    PathBuf::from(name)
}

fn timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree_source() -> Source {
        Source::Tree(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".agents"))
    }

    fn options(root: &Path) -> InstallOptions {
        InstallOptions {
            vendor: Vendor::Claude,
            scope: ReceiptScope::Project,
            root: root.to_path_buf(),
            style: None,
            force: false,
            selection: Selection::All,
        }
    }

    #[test]
    fn a_fresh_project_install_links_every_asset() {
        let root = tempfile::tempdir().unwrap();
        let plan = build_install_plan(&tree_source(), &options(root.path())).unwrap();
        assert!(plan.pending_count() > 0);
        for step in &plan.steps {
            assert!(step.is_pending(), "{} should be pending", step.id);
            assert_eq!(step.actions.len(), 1, "a linked asset is one action");
        }
    }

    #[test]
    fn an_explicit_copy_style_writes_every_file_in_a_skill() {
        let root = tempfile::tempdir().unwrap();
        let mut opts = options(root.path());
        opts.style = Some(LinkStyle::Copy);
        opts.selection = Selection::Named {
            skills: vec!["pinst".to_string()],
            commands: vec![],
        };
        let plan = build_install_plan(&tree_source(), &opts).unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert!(!plan.steps[0].actions.is_empty());
        assert!(
            plan.steps[0]
                .actions
                .iter()
                .all(|a| matches!(a, Action::Write { .. }))
        );
    }

    /// Regression for a real bug caught by hand-testing global installs: a
    /// command's copy-mode `Action::Write` target must be the file itself,
    /// not the file with an empty relative path joined on — which still
    /// appends a path separator and turns "write this file" into "write into
    /// a directory of this name".
    #[test]
    fn an_explicit_copy_style_writes_a_command_to_its_own_path_not_a_directory() {
        let root = tempfile::tempdir().unwrap();
        let mut opts = options(root.path());
        opts.style = Some(LinkStyle::Copy);
        opts.selection = Selection::Named {
            skills: vec![],
            commands: vec!["cc".to_string()],
        };
        let plan = build_install_plan(&tree_source(), &opts).unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].actions.len(), 1);
        let Action::Write { target, .. } = &plan.steps[0].actions[0] else {
            panic!("expected a Write action");
        };
        assert_eq!(
            target,
            &Vendor::Claude.commands_dir(root.path()).join("cc.md")
        );

        // And it actually executes without an ENOTDIR/EISDIR.
        let runner = crate::core::exec::Runner::new(false);
        let approve = |_: &Step| true;
        let auth = crate::core::engine::Authorizer { approve: &approve };
        let (reports, summary) = crate::core::engine::execute(&plan, &runner, &auth, &mut |_| {});
        assert_eq!(summary.failed, 0, "{reports:?}");
        assert!(
            Vendor::Claude
                .commands_dir(root.path())
                .join("cc.md")
                .is_file()
        );
    }

    #[test]
    fn a_foreign_target_is_blocked_without_force() {
        let root = tempfile::tempdir().unwrap();
        let skills_dir = Vendor::Claude.skills_dir(root.path());
        std::fs::create_dir_all(&skills_dir).unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), skills_dir.join("pinst")).unwrap();

        let mut opts = options(root.path());
        opts.selection = Selection::Named {
            skills: vec!["pinst".to_string()],
            commands: vec![],
        };
        let plan = build_install_plan(&tree_source(), &opts).unwrap();
        assert_eq!(plan.pending_count(), 0);
        assert!(matches!(
            plan.steps[0].state,
            crate::core::plan::StepState::Blocked(_)
        ));

        opts.force = true;
        let forced = build_install_plan(&tree_source(), &opts).unwrap();
        assert_eq!(forced.pending_count(), 1);
    }

    #[test]
    fn a_second_run_reports_every_step_skipped() {
        let root = tempfile::tempdir().unwrap();
        let opts = options(root.path());
        let plan = build_install_plan(&tree_source(), &opts).unwrap();
        let runner = crate::core::exec::Runner::new(false);
        let approve = |_: &Step| true;
        let auth = crate::core::engine::Authorizer { approve: &approve };
        let _ = crate::core::engine::execute(&plan, &runner, &auth, &mut |_| {});

        let second = build_install_plan(&tree_source(), &opts).unwrap();
        assert_eq!(
            second.pending_count(),
            0,
            "everything should now be skipped"
        );
    }
}
