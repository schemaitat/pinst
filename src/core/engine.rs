//! Builds plans from the manifest plus live state, and executes them.
//!
//! Execution isolates failures per step: one tool that cannot install (a
//! missing apt repo, an installer that 404s) must never abort the other
//! twenty, which is the behavior `bootstrap.sh` earned the hard way.

use std::collections::{BTreeMap, BTreeSet};

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use super::exec::{self, Runner};
use super::manifest::{Install, PostStep, Resolved, Tool};
use super::plan::{Action, Outcome, Plan, Step, StepKind, StepReport, StepState};
use super::platform::Platform;
use super::probe::ProbeResult;
use super::upgrade::UpgradeResult;

/// Builds the install plan for `tools`, in the order they are given (the
/// caller has already topologically sorted them), using each tool's
/// effective install/post-install for `platform`.
pub fn build_install_plan(
    tools: &[&Tool],
    probes: &BTreeMap<String, ProbeResult>,
    platform: Platform,
) -> Result<Plan> {
    let mut plan = Plan::default();
    let mut apt_update_added = false;
    let mut brew_update_added = false;

    for tool in tools {
        // `tools` is expected to already be platform-filtered by
        // `graph::select`; a tool that slipped through unsupported has
        // nothing to plan, so it is skipped rather than erroring the whole
        // plan over one entry.
        let Resolved::Supported(effective) = tool.resolve(platform) else {
            continue;
        };
        let installed = probes.get(&tool.name).map(|p| p.installed).unwrap_or(false);

        if installed {
            plan.push(
                Step::new(
                    format!("install:{}", tool.name),
                    StepKind::Install,
                    format!("install {}", tool.name),
                )
                .tool(&tool.name)
                .skipped("already installed"),
            );
        } else {
            let executor = exec::executor_for(effective.install);
            if let Some(reason) = executor.blocked_reason(tool, effective.install) {
                plan.push(
                    Step::new(
                        format!("install:{}", tool.name),
                        StepKind::Manual,
                        format!("install {} manually", tool.name),
                    )
                    .tool(&tool.name)
                    .blocked(reason),
                );
            } else {
                // One `apt-get update` for the whole run, emitted just before
                // the first apt install rather than once per package.
                if matches!(effective.install, Install::Apt { .. }) && !apt_update_added {
                    apt_update_added = true;
                    plan.push(
                        Step::new(
                            "apt:update",
                            StepKind::AptUpdate,
                            "refresh apt package lists",
                        )
                        .actions(vec![Action::Shell {
                            command: "sudo apt-get update -qq".to_string(),
                        }]),
                    );
                }
                // Same latch, mirrored for brew: one `brew update` per run,
                // emitted just before the first brew install.
                if matches!(effective.install, Install::Brew { .. }) && !brew_update_added {
                    brew_update_added = true;
                    plan.push(
                        Step::new("brew:update", StepKind::BrewUpdate, "refresh brew formulae")
                            .actions(vec![Action::Shell {
                                command: "brew update".to_string(),
                            }]),
                    );
                }

                let confirm = requires_confirmation(effective.install);
                plan.push(
                    Step::new(
                        format!("install:{}", tool.name),
                        StepKind::Install,
                        format!("install {}", tool.name),
                    )
                    .tool(&tool.name)
                    .actions(executor.install(tool, effective.install)?)
                    .confirm(confirm),
                );
            }
        }

        push_post_install(&mut plan, tool, effective.post_install);
    }

    Ok(plan)
}

/// Builds the upgrade plan for tools that a prior upgrade check flagged,
/// using each tool's effective install for `platform`.
pub fn build_upgrade_plan(
    tools: &[&Tool],
    upgrades: &[UpgradeResult],
    platform: Platform,
) -> Result<Plan> {
    let by_name: BTreeMap<&str, &UpgradeResult> =
        upgrades.iter().map(|u| (u.tool.as_str(), u)).collect();

    let mut plan = Plan::default();
    let mut apt_update_added = false;
    let mut brew_update_added = false;

    for tool in tools {
        let Resolved::Supported(effective) = tool.resolve(platform) else {
            continue;
        };
        let step_id = format!("upgrade:{}", tool.name);
        let Some(result) = by_name.get(tool.name.as_str()) else {
            continue;
        };

        if !result.upgrade_available {
            let reason = match (&result.current, &result.latest) {
                (Some(current), _) => format!("up to date ({current})"),
                (None, _) => "no version information".to_string(),
            };
            plan.push(
                Step::new(step_id, StepKind::Upgrade, format!("upgrade {}", tool.name))
                    .tool(&tool.name)
                    .skipped(reason),
            );
            continue;
        }

        let executor = exec::executor_for(effective.install);
        if let Some(reason) = executor.blocked_reason(tool, effective.install) {
            plan.push(
                Step::new(
                    step_id,
                    StepKind::Manual,
                    format!("upgrade {} manually", tool.name),
                )
                .tool(&tool.name)
                .blocked(reason),
            );
            continue;
        }

        if matches!(effective.install, Install::Apt { .. }) && !apt_update_added {
            apt_update_added = true;
            plan.push(
                Step::new(
                    "apt:update",
                    StepKind::AptUpdate,
                    "refresh apt package lists",
                )
                .actions(vec![Action::Shell {
                    command: "sudo apt-get update -qq".to_string(),
                }]),
            );
        }
        if matches!(effective.install, Install::Brew { .. }) && !brew_update_added {
            brew_update_added = true;
            plan.push(
                Step::new("brew:update", StepKind::BrewUpdate, "refresh brew formulae").actions(
                    vec![Action::Shell {
                        command: "brew update".to_string(),
                    }],
                ),
            );
        }

        let latest = result
            .latest
            .clone()
            .unwrap_or_else(|| "latest".to_string());
        plan.push(
            Step::new(
                step_id,
                StepKind::Upgrade,
                format!("upgrade {} to {latest}", tool.name),
            )
            .tool(&tool.name)
            .actions(executor.upgrade(tool, effective.install)?)
            .confirm(requires_confirmation(effective.install)),
        );
    }

    Ok(plan)
}

fn push_post_install(plan: &mut Plan, tool: &Tool, post_install: &[PostStep]) {
    for (index, post) in post_install.iter().enumerate() {
        let description = if post.description.is_empty() {
            post.command.clone()
        } else {
            post.description.clone()
        };
        plan.push(
            Step::new(
                format!("post:{}:{index}", tool.name),
                StepKind::PostInstall,
                description,
            )
            .tool(&tool.name)
            .actions(vec![Action::Shell {
                command: post.command.clone(),
            }])
            .condition(post.skip_if.clone())
            .confirm(post.confirm),
        );
    }
}

fn requires_confirmation(install: &Install) -> bool {
    match install {
        Install::GithubRelease { confirm, .. } => *confirm,
        _ => false,
    }
}

/// How a step's authorization is decided, supplied by the CLI layer so the
/// engine never touches a terminal itself.
pub struct Authorizer<'a> {
    pub approve: &'a dyn Fn(&Step) -> bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema, Default)]
pub struct ExecSummary {
    pub ran: usize,
    pub skipped: usize,
    pub failed: usize,
    pub blocked: usize,
    pub needs_confirmation: usize,
    pub would_run: usize,
}

/// Executes a plan, isolating failures. Returns one report per step plus the
/// tallies an agent branches on.
pub fn execute(
    plan: &Plan,
    runner: &Runner,
    auth: &Authorizer<'_>,
    on_step: &mut dyn FnMut(&StepReport),
) -> (Vec<StepReport>, ExecSummary) {
    let mut reports = Vec::with_capacity(plan.steps.len());
    let mut summary = ExecSummary::default();
    let mut unavailable: BTreeSet<&str> = BTreeSet::new();

    for step in &plan.steps {
        // A tool whose install did not succeed must not have its post-install
        // steps run: `chsh -s "$(command -v zsh)"` with no zsh on the box
        // would set an empty login shell, and the fd symlink step would link
        // from an empty path.
        let report = match (&step.tool, step.kind) {
            (Some(tool), StepKind::PostInstall) if unavailable.contains(tool.as_str()) => {
                summary.skipped += 1;
                StepReport::from_step(
                    step,
                    Outcome::Skipped,
                    Some(format!("{tool} was not installed")),
                )
            }
            _ => execute_step(step, runner, auth, &mut summary),
        };

        // `Manual` counts as an install step too — that is the kind a blocked
        // tool gets, and it is precisely the case where post-install steps
        // must not fire.
        if matches!(step.kind, StepKind::Install | StepKind::Manual)
            && matches!(
                report.outcome,
                Outcome::Failed | Outcome::Blocked | Outcome::NeedsConfirmation
            )
            && let Some(tool) = &step.tool
        {
            unavailable.insert(tool.as_str());
        }

        on_step(&report);
        reports.push(report);
    }

    (reports, summary)
}

fn execute_step(
    step: &Step,
    runner: &Runner,
    auth: &Authorizer<'_>,
    summary: &mut ExecSummary,
) -> StepReport {
    match &step.state {
        StepState::Skipped(reason) => {
            summary.skipped += 1;
            return StepReport::from_step(step, Outcome::Skipped, Some(reason.clone()));
        }
        StepState::Blocked(reason) => {
            summary.blocked += 1;
            return StepReport::from_step(step, Outcome::Blocked, Some(reason.clone()));
        }
        StepState::Pending => {}
    }

    // Conditions are evaluated here, not at plan time: a post-install step's
    // check often only becomes true once the tool it belongs to is installed.
    if let Some(condition) = &step.condition
        && exec::check(condition)
    {
        summary.skipped += 1;
        return StepReport::from_step(
            step,
            Outcome::Skipped,
            Some(format!("condition already satisfied: {condition}")),
        );
    }

    if step.confirm && !(auth.approve)(step) {
        summary.needs_confirmation += 1;
        return StepReport::from_step(
            step,
            Outcome::NeedsConfirmation,
            Some("needs authorization; re-run with --yes".to_string()),
        );
    }

    if runner.dry_run {
        summary.would_run += 1;
        return StepReport::from_step(step, Outcome::WouldRun, None);
    }

    for action in &step.actions {
        if let Err(err) = runner.run(action) {
            summary.failed += 1;
            return StepReport::from_step(step, Outcome::Failed, Some(format!("{err:#}")));
        }
    }

    summary.ran += 1;
    StepReport::from_step(step, Outcome::Ran, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::{Selection, select};
    use crate::core::manifest;
    use crate::core::platform::Platform;

    fn probes(installed: &[&str], all: &[&Tool]) -> BTreeMap<String, ProbeResult> {
        all.iter()
            .map(|tool| {
                (
                    tool.name.clone(),
                    ProbeResult {
                        tool: tool.name.clone(),
                        installed: installed.contains(&tool.name.as_str()),
                        version: installed
                            .contains(&tool.name.as_str())
                            .then(|| "1.0.0".to_string()),
                        path: None,
                    },
                )
            })
            .collect()
    }

    fn approve_all(_: &Step) -> bool {
        true
    }

    #[test]
    fn plan_follows_dependency_order() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(&manifest, &Selection::default(), Platform::Linux)
            .unwrap()
            .tools;
        let plan = build_install_plan(&tools, &probes(&[], &tools), Platform::Linux).unwrap();

        let ids: Vec<&str> = plan
            .steps
            .iter()
            .filter(|s| s.kind == StepKind::Install)
            .map(|s| s.id.as_str())
            .collect();
        let pos = |id: &str| ids.iter().position(|i| *i == id).unwrap();

        assert!(pos("install:zsh") < pos("install:oh-my-zsh"));
        assert!(pos("install:oh-my-zsh") < pos("install:zsh-autosuggestions"));
        assert!(pos("install:rust") < pos("install:tree-sitter-cli"));
        assert!(pos("install:nvm") < pos("install:node"));
    }

    #[test]
    fn already_installed_tools_are_skipped() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(
            &manifest,
            &Selection {
                names: vec!["direnv".to_string()],
                ..Default::default()
            },
            Platform::Linux,
        )
        .unwrap()
        .tools;

        let plan =
            build_install_plan(&tools, &probes(&["direnv"], &tools), Platform::Linux).unwrap();
        let step = plan
            .steps
            .iter()
            .find(|s| s.id == "install:direnv")
            .unwrap();
        assert!(matches!(step.state, StepState::Skipped(_)));
        assert_eq!(
            plan.pending_count(),
            0,
            "a provisioned machine plans no work"
        );
    }

    #[test]
    fn apt_update_is_emitted_once_before_the_first_apt_install() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(&manifest, &Selection::default(), Platform::Linux)
            .unwrap()
            .tools;
        let plan = build_install_plan(&tools, &probes(&[], &tools), Platform::Linux).unwrap();

        let updates: Vec<usize> = plan
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == StepKind::AptUpdate)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(updates.len(), 1, "one refresh per run, not per package");

        let first_apt = plan
            .steps
            .iter()
            .position(|s| {
                s.kind == StepKind::Install
                    && s.actions.iter().any(|a| match a {
                        Action::Shell { command } => command.contains("apt-get install"),
                        _ => false,
                    })
            })
            .unwrap();
        assert!(updates[0] < first_apt);
    }

    // TEST-011: mirrors the apt test above, for brew — one `brew update`
    // per run, strictly before the first brew install step.
    #[test]
    fn brew_update_is_emitted_once_before_the_first_brew_install() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(&manifest, &Selection::default(), Platform::MacOS)
            .unwrap()
            .tools;
        let plan = build_install_plan(&tools, &probes(&[], &tools), Platform::MacOS).unwrap();

        let updates: Vec<usize> = plan
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == StepKind::BrewUpdate)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(updates.len(), 1, "one refresh per run, not per package");

        let first_brew = plan
            .steps
            .iter()
            .position(|s| {
                s.kind == StepKind::Install
                    && s.actions.iter().any(|a| match a {
                        Action::Shell { command } => command.contains("brew install"),
                        _ => false,
                    })
            })
            .unwrap();
        assert!(updates[0] < first_brew);
    }

    // TEST-012: pinst plan --platform macos orders homebrew before every
    // tool that requires it, and pinst plan --platform linux drops it as
    // unsupported.
    #[test]
    fn homebrew_orders_before_its_dependents_and_is_unsupported_on_linux() {
        let manifest = manifest::embedded().unwrap();

        let macos = select(&manifest, &Selection::default(), Platform::MacOS).unwrap();
        let names: Vec<&str> = macos.tools.iter().map(|t| t.name.as_str()).collect();
        let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
        assert!(pos("homebrew") < pos("ripgrep"));
        assert!(pos("homebrew") < pos("direnv"));
        assert!(pos("homebrew") < pos("delta"));

        let linux = select(&manifest, &Selection::default(), Platform::Linux).unwrap();
        assert!(
            linux.unsupported.iter().any(|u| u.name == "homebrew"),
            "homebrew has no linux install path"
        );
    }

    // TEST-016: a macOS plan contains no apt step and no sudo outside the
    // Homebrew bootstrap, and reports build-essential as unsupported with
    // the xcode-select note.
    #[test]
    fn macos_plan_has_no_apt_and_names_the_xcode_clt_remediation() {
        let manifest = manifest::embedded().unwrap();
        let selected = select(&manifest, &Selection::default(), Platform::MacOS).unwrap();
        let plan = build_install_plan(
            &selected.tools,
            &probes(&[], &selected.tools),
            Platform::MacOS,
        )
        .unwrap();

        assert!(
            plan.steps.iter().all(|s| s.kind != StepKind::AptUpdate),
            "a macOS plan should contain no apt-get step"
        );
        for step in &plan.steps {
            for action in &step.actions {
                if let Action::Shell { command } = action {
                    // The only sudo in this manifest's macOS surface is
                    // Homebrew's own bootstrap, which is a `shell` install
                    // step, not one this loop should be finding sudo in —
                    // every *brew* step must be sudo-free (RISK-002).
                    if step.tool.as_deref() != Some("homebrew") {
                        assert!(!command.contains("sudo"), "{}: {command}", step.id);
                    }
                }
            }
        }

        let be = selected
            .unsupported
            .iter()
            .find(|u| u.name == "build-essential")
            .expect("build-essential has no macOS install path");
        assert!(be.note.contains("xcode-select"), "{}", be.note);
    }

    // TEST-017: the resolved macOS neovim entry is a brew install with no
    // confirm gate and no /opt destination.
    #[test]
    fn macos_neovim_is_a_brew_install_with_no_confirm_or_opt() {
        let manifest = manifest::embedded().unwrap();
        let neovim = manifest.tool("neovim").unwrap();
        let Resolved::Supported(effective) = neovim.resolve(Platform::MacOS) else {
            panic!("neovim has a macos override");
        };
        match effective.install {
            Install::Brew { formulae, .. } => assert_eq!(formulae, &["neovim".to_string()]),
            other => panic!("expected a brew install, got {other:?}"),
        }
    }

    // TEST-018: the resolved macOS pinst entry names the aarch64 Darwin
    // asset, and the Linux one is unchanged.
    #[test]
    fn macos_pinst_asset_is_the_aarch64_darwin_tarball() {
        let manifest = manifest::embedded().unwrap();
        let pinst = manifest.tool("pinst").unwrap();

        let Resolved::Supported(linux) = pinst.resolve(Platform::Linux) else {
            panic!("pinst is supported on linux");
        };
        match linux.install {
            Install::GithubRelease { asset, .. } => {
                assert_eq!(asset, "pinst-x86_64-unknown-linux-musl.tar.gz")
            }
            other => panic!("expected a github_release install, got {other:?}"),
        }

        let Resolved::Supported(macos) = pinst.resolve(Platform::MacOS) else {
            panic!("pinst is supported on macos");
        };
        match macos.install {
            Install::GithubRelease { asset, .. } => {
                assert_eq!(asset, "pinst-aarch64-apple-darwin.tar.gz")
            }
            other => panic!("expected a github_release install, got {other:?}"),
        }
    }

    // TEST-019: zsh's chsh step is skipped when the resolved zsh is absent
    // from /etc/shells — exercised against a temporary stand-in file rather
    // than the real /etc/shells (LESSON-018).
    #[test]
    fn chsh_is_skipped_when_zsh_is_not_listed_in_etc_shells() {
        let manifest = manifest::embedded().unwrap();
        let zsh = manifest.tool("zsh").unwrap();
        let post = &zsh.post_install[0];
        let skip_if = post.skip_if.as_deref().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let fake_shells = dir.path().join("shells");
        std::fs::write(&fake_shells, "/bin/bash\n").unwrap();

        // Substitute the real /etc/shells for the fake one, and a fake zsh
        // path that is not listed in it, and a $SHELL that is not zsh.
        let check = skip_if
            .replace("/etc/shells", fake_shells.to_str().unwrap())
            .replace("\"$(command -v zsh)\"", "/usr/local/bin/zsh");
        let status = std::process::Command::new("sh")
            .env("SHELL", "/bin/bash")
            .arg("-c")
            .arg(&check)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "skip_if should succeed (skip the chsh step) when zsh is absent from /etc/shells: {check}"
        );
    }

    #[test]
    fn manual_tools_are_blocked_with_instructions() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(
            &manifest,
            &Selection {
                names: vec!["git-credential-manager".to_string()],
                ..Default::default()
            },
            Platform::Linux,
        )
        .unwrap()
        .tools;
        let plan = build_install_plan(&tools, &probes(&[], &tools), Platform::Linux).unwrap();
        let step = plan
            .steps
            .iter()
            .find(|s| s.id == "install:git-credential-manager")
            .unwrap();
        match &step.state {
            StepState::Blocked(note) => assert!(note.contains("git-credential-manager")),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn post_install_steps_do_not_run_when_their_tool_did_not_install() {
        // The dangerous case: `chsh -s "$(command -v zsh)"` after a failed
        // zsh install would set an empty login shell.
        let mut plan = Plan::default();
        plan.push(
            Step::new("install:ghost", StepKind::Manual, "install ghost")
                .tool("ghost")
                .blocked("install it yourself"),
        );
        plan.push(
            Step::new("post:ghost:0", StepKind::PostInstall, "post step")
                .tool("ghost")
                .actions(vec![Action::Shell {
                    command: "touch /tmp/pinst-post-must-not-run".to_string(),
                }]),
        );
        plan.push(
            Step::new("install:other", StepKind::Install, "install other")
                .tool("other")
                .actions(vec![Action::Shell {
                    command: "false".to_string(),
                }]),
        );
        plan.push(
            Step::new("post:other:0", StepKind::PostInstall, "other post step")
                .tool("other")
                .actions(vec![Action::Shell {
                    command: "touch /tmp/pinst-post-must-not-run-2".to_string(),
                }]),
        );

        let _ = std::fs::remove_file("/tmp/pinst-post-must-not-run");
        let _ = std::fs::remove_file("/tmp/pinst-post-must-not-run-2");

        let (reports, _) = execute(
            &plan,
            &Runner::new(false),
            &Authorizer {
                approve: &approve_all,
            },
            &mut |_| {},
        );

        assert_eq!(
            reports[1].outcome,
            Outcome::Skipped,
            "blocked tool's post step"
        );
        assert_eq!(
            reports[3].outcome,
            Outcome::Skipped,
            "failed tool's post step"
        );
        assert!(!std::path::Path::new("/tmp/pinst-post-must-not-run").exists());
        assert!(!std::path::Path::new("/tmp/pinst-post-must-not-run-2").exists());
    }

    #[test]
    fn dry_run_reports_would_run_and_mutates_nothing() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(
            &manifest,
            &Selection {
                names: vec!["curl".to_string()],
                ..Default::default()
            },
            Platform::Linux,
        )
        .unwrap()
        .tools;
        let plan = build_install_plan(&tools, &probes(&[], &tools), Platform::Linux).unwrap();

        let runner = Runner::new(true);
        let auth = Authorizer {
            approve: &approve_all,
        };
        let (reports, summary) = execute(&plan, &runner, &auth, &mut |_| {});

        assert!(summary.ran == 0 && summary.would_run > 0);
        assert!(reports.iter().all(|r| r.outcome != Outcome::Ran));
    }

    #[test]
    fn failures_are_isolated_and_later_steps_still_run() {
        let mut plan = Plan::default();
        plan.push(
            Step::new("t:fail", StepKind::Install, "failing step").actions(vec![Action::Shell {
                command: "exit 3".to_string(),
            }]),
        );
        plan.push(
            Step::new("t:after", StepKind::Install, "later step").actions(vec![Action::Shell {
                command: "true".to_string(),
            }]),
        );

        let runner = Runner::new(false);
        let auth = Authorizer {
            approve: &approve_all,
        };
        let (reports, summary) = execute(&plan, &runner, &auth, &mut |_| {});

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.ran, 1, "the step after a failure still ran");
        assert_eq!(reports[0].outcome, Outcome::Failed);
        assert_eq!(reports[1].outcome, Outcome::Ran);
    }

    #[test]
    fn unapproved_confirmation_gates_block_the_step() {
        let mut plan = Plan::default();
        plan.push(
            Step::new("t:risky", StepKind::Install, "risky step")
                .actions(vec![Action::Shell {
                    command: "touch /tmp/pinst-should-not-exist".to_string(),
                }])
                .confirm(true),
        );

        let deny = |_: &Step| false;
        let (reports, summary) = execute(
            &plan,
            &Runner::new(false),
            &Authorizer { approve: &deny },
            &mut |_| {},
        );

        assert_eq!(summary.needs_confirmation, 1);
        assert_eq!(reports[0].outcome, Outcome::NeedsConfirmation);
        assert!(!std::path::Path::new("/tmp/pinst-should-not-exist").exists());
    }

    #[test]
    fn satisfied_conditions_skip_the_step_at_execution_time() {
        let mut plan = Plan::default();
        plan.push(
            Step::new("t:cond", StepKind::PostInstall, "conditional step")
                .actions(vec![Action::Shell {
                    command: "touch /tmp/pinst-condition-should-not-run".to_string(),
                }])
                .condition(Some("true".to_string())),
        );

        let (reports, summary) = execute(
            &plan,
            &Runner::new(false),
            &Authorizer {
                approve: &approve_all,
            },
            &mut |_| {},
        );

        assert_eq!(summary.skipped, 1);
        assert_eq!(reports[0].outcome, Outcome::Skipped);
        assert!(!std::path::Path::new("/tmp/pinst-condition-should-not-run").exists());
    }
}
