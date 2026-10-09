//! Locked Nix environments. Build inputs are explicit; no channel/registry
//! lookup or lock update is allowed during convergence.
use std::path::{Path, PathBuf};

use color_eyre::eyre::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

use super::exec::quote;
use super::plan::{Action, Plan, Step, StepKind};
use super::usage;

pub fn state_dir(home: &Path) -> PathBuf {
    home.join(".local/state/pinst/environment")
}

const OWNER: &str = ".config/pinst/environment.json";

pub fn is_managed(home: &Path) -> bool {
    // Even a dangling marker must stop the legacy writer; a damaged Nix
    // generation is not permission to overwrite its files from the binary.
    std::fs::symlink_metadata(home.join(OWNER)).is_ok()
}

pub fn ensure_legacy(home: &Path) -> Result<()> {
    if is_managed(home) {
        return Err(usage(
            "Home Manager owns this environment; use pinst environment build/apply/status instead of legacy install/config/apply/update",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub schema_version: u32,
    pub owner: String,
    pub home: PathBuf,
    pub external: Vec<ExternalTool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalTool {
    pub name: String,
    pub reason: String,
    pub remediation: String,
}

fn metadata(generation: &Path, home: &Path) -> Result<Metadata> {
    let path = generation.join("home-files").join(OWNER);
    let metadata: Metadata = serde_json::from_slice(
        &std::fs::read(&path)
            .with_context(|| format!("reading generation metadata {}", path.display()))?,
    )?;
    if metadata.schema_version != 1 || metadata.owner != "home-manager" || metadata.home != home {
        bail!(
            "generation is not a pinst Home Manager environment for {}",
            home.display()
        );
    }
    if !generation.join("activate").is_file() {
        bail!("generation has no activation script");
    }
    Ok(metadata)
}

fn managed_files(generation: &Path) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                walk(root, &entry.path(), files)?;
            } else {
                files.push(entry.path().strip_prefix(root)?.to_path_buf());
            }
        }
        Ok(())
    }
    let root = generation.join("home-files");
    let mut files = Vec::new();
    walk(&root, &root, &mut files)?;
    files.sort();
    Ok(files)
}

fn conflicts(home: &Path, files: &[PathBuf], owned: bool) -> Result<Vec<PathBuf>> {
    let mut found = BTreeSet::new();
    for file in files {
        let mut relative = PathBuf::new();
        for component in file.components() {
            relative.push(component);
            let target = home.join(&relative);
            let Ok(meta) = std::fs::symlink_metadata(&target) else {
                continue;
            };
            if relative == *file || meta.file_type().is_symlink() {
                if owned
                    && meta.file_type().is_symlink()
                    && target
                        .canonicalize()
                        .is_ok_and(|p| p.starts_with("/nix/store"))
                {
                    break;
                }
                if relative != *file
                    && ![
                        ".config/nvim",
                        ".config/git",
                        ".config/herdr",
                        ".config/zsh",
                    ]
                    .iter()
                    .any(|allowed| relative == Path::new(allowed))
                {
                    bail!(
                        "config parent {} is a symlink; resolve it before migrating",
                        target.display()
                    );
                }
                found.insert(relative.clone());
                break;
            }
        }
    }
    let paths: Vec<PathBuf> = found.into_iter().collect();
    Ok(paths
        .iter()
        .filter(|path| {
            !paths
                .iter()
                .any(|parent| *path != parent && path.starts_with(parent))
        })
        .cloned()
        .collect())
}

/// Activate a previously built generation. The canonical store path is
/// captured in the plan, so changing the build symlink cannot change the run.
pub fn activation_plan(home: &Path, generation: &Path, migrate: bool) -> Result<Plan> {
    let generation = generation
        .canonicalize()
        .context("resolve the built generation")?;
    if !generation.starts_with("/nix/store") {
        return Err(usage("activation requires a built /nix/store generation"));
    }
    activation_plan_for(home, &generation, migrate)
}

fn activation_plan_for(home: &Path, generation: &Path, migrate: bool) -> Result<Plan> {
    metadata(generation, home)?;
    let files = managed_files(generation)?;
    let state = state_dir(home);
    let current = state.join("current").canonicalize().ok();
    let collisions = conflicts(home, &files, is_managed(home))?;
    let mut plan = Plan::default();
    if !migrate && !collisions.is_empty() {
        plan.push(Step::new("environment:activate", StepKind::Environment, "activate Home Manager generation")
            .blocked(format!("existing files: {}. Inspect them, then use --migrate to preserve originals and snapshots before activation",
                collisions.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))));
        return Ok(plan);
    }
    let healthy = current.as_deref() == Some(generation)
        && profile_health(home, generation)?
            .iter()
            .all(|(_, linked)| *linked)
        && files.iter().all(|relative| {
            home.join(relative).exists()
                && home.join(relative).canonicalize().ok()
                    == generation
                        .join("home-files")
                        .join(relative)
                        .canonicalize()
                        .ok()
        });
    if healthy {
        plan.push(
            Step::new(
                "environment:activate",
                StepKind::Environment,
                "activate Home Manager generation",
            )
            .skipped("this generation is already active and its files are linked"),
        );
        return Ok(plan);
    }
    let mut actions = vec![Action::Shell {
        command: format!("mkdir -p {}", quote(&state.to_string_lossy())),
    }];
    if !collisions.is_empty() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let backup = state
            .join("migrations")
            .join(format!("{nonce}-{}", std::process::id()));
        let receipt = serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1, "home": home, "generation": generation,
            "originals": collisions, "managed": files,
            "note": "original/ preserves original files and symlinks; snapshot/ preserves their live contents"
        }))?;
        actions.push(Action::Write {
            target: backup.join("receipt.json"),
            bytes: receipt.len(),
            content: Arc::new(receipt),
        });
        let identity = home.join(".config/pinst/git-identity");
        if !identity.exists()
            && let Some(content) = git_identity(home)
        {
            actions.push(Action::Write {
                target: identity,
                bytes: content.len(),
                content: Arc::new(content),
            });
        }
        for relative in &collisions {
            let target = home.join(relative);
            let snapshot = backup.join("snapshot").join(relative);
            let original = backup.join("original").join(relative);
            // Snapshot first, then move the original link/file. Relative links
            // regain their original meaning when moved back to the home path.
            let mut command = format!(
                "mkdir -p {} {}",
                quote(&snapshot.parent().unwrap().to_string_lossy()),
                quote(&original.parent().unwrap().to_string_lossy())
            );
            if target.exists() {
                command.push_str(&format!(
                    " && cp -RL {} {}",
                    quote(&target.to_string_lossy()),
                    quote(&snapshot.to_string_lossy())
                ));
            }
            actions.push(Action::Shell { command });
            actions.push(Action::Backup {
                path: target,
                to: original,
            });
        }
    }
    // Retain both store paths as GC roots; activation failure leaves the
    // previous successful generation and all migration evidence available.
    if let Some(previous) = current.as_ref().filter(|p| p.as_path() != generation) {
        actions.push(root_action(previous, &state.join("previous")));
    }
    actions.push(root_action(generation, &state.join("candidate")));
    actions.push(Action::Shell {
        command: quote(&generation.join("activate").to_string_lossy()),
    });
    actions.push(root_action(generation, &state.join("current")));
    plan.push(
        Step::new(
            "environment:activate",
            StepKind::Environment,
            "activate Home Manager generation",
        )
        .actions(actions)
        .confirm(true),
    );
    Ok(plan)
}

fn git_identity(home: &Path) -> Option<Vec<u8>> {
    let file = home.join(".gitconfig");
    if !file.is_file() {
        return None;
    }
    let mut content = String::from("[user]\n");
    let mut found = false;
    for key in ["name", "email"] {
        let output = std::process::Command::new("git")
            .arg("config")
            .arg("--file")
            .arg(&file)
            .arg("--no-includes")
            .arg("--get")
            .arg(format!("user.{key}"))
            .output()
            .ok()?;
        if output.status.success() {
            let value = String::from_utf8(output.stdout).ok()?;
            let value = value
                .trim_end_matches('\n')
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\t', "\\t");
            content.push_str(&format!("\t{key} = \"{value}\"\n"));
            found = true;
        }
    }
    found.then(|| content.into_bytes())
}

fn root_action(generation: &Path, root: &Path) -> Action {
    Action::Shell {
        command: format!(
            "nix-store --realise {} --add-root {} --indirect",
            quote(&generation.to_string_lossy()),
            quote(&root.to_string_lossy())
        ),
    }
}

pub fn rollback_plan(home: &Path) -> Result<Plan> {
    activation_plan(home, &state_dir(home).join("previous"), false)
}

pub fn status(home: &Path) -> Result<(Vec<serde_json::Value>, bool)> {
    let current = state_dir(home).join("current");
    if !current.exists() {
        return Ok((
            vec![
                serde_json::json!({ "id": "environment.missing", "state": "missing",
            "remediation": "pinst environment build --configuration <name>, then pinst environment apply" }),
            ],
            false,
        ));
    }
    let generation = current.canonicalize()?;
    let metadata = metadata(&generation, home)?;
    let mut healthy = true;
    let mut items = Vec::new();
    for (name, linked) in profile_health(home, &generation)? {
        healthy &= linked;
        items.push(
            serde_json::json!({ "id": format!("environment.package.{name}"),
            "state": if linked { "linked" } else { "drifted" }, "generation": generation }),
        );
    }
    for relative in managed_files(&generation)? {
        let target = home.join(&relative);
        let linked = target.canonicalize().ok()
            == generation
                .join("home-files")
                .join(&relative)
                .canonicalize()
                .ok()
            && target.exists();
        healthy &= linked;
        items.push(serde_json::json!({ "id": format!("environment.file.{}", relative.display()),
            "target": target, "state": if linked { "linked" } else { "drifted" }, "generation": generation }));
    }
    for tool in metadata.external {
        let path = which::which_in(&tool.name, Some(super::exec::search_path()), home).ok();
        items.push(
            serde_json::json!({ "id": format!("environment.external.{}", tool.name),
            "state": "external", "installed": path.is_some(), "path": path,
            "reason": tool.reason, "remediation": tool.remediation }),
        );
    }
    Ok((items, healthy))
}

fn profile_health(home: &Path, generation: &Path) -> Result<Vec<(String, bool)>> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(generation.join("home-path/bin"))? {
        let entry = entry?;
        let name = entry.file_name();
        let expected = entry.path().canonicalize()?;
        let actual = home
            .join(".nix-profile/bin")
            .join(&name)
            .canonicalize()
            .ok();
        items.push((
            name.to_string_lossy().into_owned(),
            actual.as_deref() == Some(expected.as_path()),
        ));
    }
    items.sort();
    Ok(items)
}

pub fn build_plan(flake: &Path, configuration: Option<&str>, output: &Path) -> Result<Plan> {
    let flake = flake
        .canonicalize()
        .with_context(|| format!("opening flake {}", flake.display()))?;
    if !flake.join("flake.nix").is_file() || !flake.join("flake.lock").is_file() {
        return Err(usage("a flake.nix and committed flake.lock are required"));
    }
    let _: serde_json::Value = serde_json::from_slice(&std::fs::read(flake.join("flake.lock"))?)
        .context("invalid flake.lock JSON")?;
    if flake.to_string_lossy().contains(['#', '?']) {
        return Err(usage("flake paths containing # or ? are not supported"));
    }
    let attribute = match configuration {
        None => "toolchain".into(),
        Some(name) => {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            {
                return Err(usage(
                    "configuration names must contain only letters, digits, - or _",
                ));
            }
            format!("homeConfigurations.\"{name}\".activationPackage")
        }
    };
    let installable = format!("{}#{attribute}", flake.display());
    let output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()?.join(output)
    };
    let parent = output
        .parent()
        .ok_or_else(|| usage("build output needs a parent directory"))?;
    let mut plan = Plan::default();
    plan.push(Step::new("environment:build", StepKind::Environment, "build locked Nix environment")
        .actions(vec![Action::Shell { command: format!(
            "mkdir -p {} && nix --extra-experimental-features 'nix-command flakes' build {} --no-update-lock-file --out-link {} --json",
            quote(&parent.to_string_lossy()), quote(&installable), quote(&output.to_string_lossy())) }]));
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{engine, exec::Runner, plan::Outcome};

    fn fixture(root: &Path) -> (PathBuf, PathBuf) {
        let home = root.join("home");
        let generation = root.join("generation");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(generation.join("home-files/.config/pinst")).unwrap();
        std::fs::write(generation.join("home-files").join(OWNER),
            serde_json::to_vec(&serde_json::json!({"schema_version":1,"owner":"home-manager","home":home,"external":[]})).unwrap()).unwrap();
        std::fs::write(generation.join("home-files/.zshrc"), "new shell").unwrap();
        std::fs::write(generation.join("activate"), "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            generation.join("activate"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        (home, generation)
    }

    #[test]
    fn migration_preserves_live_symlink_bytes_and_original_link() {
        let root = tempfile::tempdir().unwrap();
        let (home, generation) = fixture(root.path());
        std::fs::create_dir(home.join("dotfiles")).unwrap();
        std::fs::write(home.join("dotfiles/zshrc"), "uncommitted live edits").unwrap();
        std::os::unix::fs::symlink("dotfiles/zshrc", home.join(".zshrc")).unwrap();
        let blocked = activation_plan_for(&home, &generation, false).unwrap();
        assert!(matches!(
            blocked.steps[0].state,
            super::super::plan::StepState::Blocked(_)
        ));
        let plan = activation_plan_for(&home, &generation, true).unwrap();
        // Exercise the real snapshot and backup actions; stop at Nix's first
        // GC-root action. Actual Nix activation is covered by container e2e.
        for action in &plan.steps[0].actions {
            if matches!(action, Action::Shell { command } if command.starts_with("nix-store")) {
                break;
            }
            Runner::new(false).run(action).unwrap();
        }
        let backup = std::fs::read_dir(state_dir(&home).join("migrations"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read_to_string(backup.join("snapshot/.zshrc")).unwrap(),
            "uncommitted live edits"
        );
        assert_eq!(
            std::fs::read_link(backup.join("original/.zshrc")).unwrap(),
            Path::new("dotfiles/zshrc")
        );
        assert_eq!(
            std::fs::read_to_string(home.join("dotfiles/zshrc")).unwrap(),
            "uncommitted live edits"
        );
        std::fs::rename(backup.join("original/.zshrc"), home.join(".zshrc")).unwrap();
        assert_eq!(
            std::fs::read_to_string(home.join(".zshrc")).unwrap(),
            "uncommitted live edits"
        );
    }

    #[test]
    fn unsafe_config_parent_and_wrong_home_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let (home, generation) = fixture(root.path());
        assert!(activation_plan_for(&root.path().join("other"), &generation, true).is_err());
        std::fs::create_dir(home.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(home.join("elsewhere"), home.join(".config")).unwrap();
        assert!(activation_plan_for(&home, &generation, true).is_err());
    }

    #[test]
    fn even_dangling_ownership_markers_block_legacy_writers() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".config/pinst")).unwrap();
        std::os::unix::fs::symlink("missing", root.path().join(OWNER)).unwrap();
        assert!(ensure_legacy(root.path()).is_err());
    }

    #[test]
    fn preview_preserves_home_and_requires_a_lock() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("state/build");
        assert!(build_plan(root.path(), None, &output).is_err());
        std::fs::write(root.path().join("flake.nix"), "{}").unwrap();
        std::fs::write(root.path().join("flake.lock"), "{}").unwrap();
        let plan = build_plan(root.path(), Some("test-linux"), &output).unwrap();
        let (reports, _) = engine::execute(
            &plan,
            &Runner::new(true),
            &engine::Authorizer { approve: &|_| true },
            &mut |_| {},
        );
        assert_eq!(reports[0].outcome, Outcome::WouldRun);
        assert!(!output.parent().unwrap().exists());
        assert!(reports[0].commands[0].contains("--no-update-lock-file"));
        assert!(build_plan(root.path(), Some("bad;name"), &output).is_err());
    }
}
