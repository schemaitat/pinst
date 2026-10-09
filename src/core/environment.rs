//! Locked Nix environments. Build inputs are explicit; no channel/registry
//! lookup or lock update is allowed during convergence.
use std::path::{Path, PathBuf};

use color_eyre::eyre::{Context, Result};

use super::exec::quote;
use super::plan::{Action, Plan, Step, StepKind};
use super::usage;

pub fn state_dir(home: &Path) -> PathBuf {
    home.join(".local/state/pinst/environment")
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
