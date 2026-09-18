//! Builds plans from the manifest plus live state, and executes them.
//!
//! Execution isolates failures per step: one tool that cannot install (a
//! missing apt repo, an installer that 404s) must never abort the other
//! twenty, which is the behavior `bootstrap.sh` earned the hard way.

use std::collections::BTreeMap;

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use super::exec::{self, Runner};
use super::manifest::{Install, Tool};
use super::plan::{Action, Outcome, Plan, Step, StepKind, StepReport, StepState};
use super::probe::ProbeResult;
use super::upgrade::UpgradeResult;

/// Builds the install plan for `tools`, in the order they are given (the
/// caller has already topologically sorted them).
pub fn build_install_plan(tools: &[&Tool], probes: &BTreeMap<String, ProbeResult>) -> Result<Plan> {
    let mut plan = Plan::default();
    let mut apt_update_added = false;

    for tool in tools {
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
            let executor = exec::executor_for(&tool.install);
            if let Some(reason) = executor.blocked_reason(tool) {
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
                if matches!(tool.install, Install::Apt { .. }) && !apt_update_added {
                    apt_update_added = true;
                    plan.push(
                        Step::new("apt:update", StepKind::AptUpdate, "refresh apt package lists")
                            .actions(vec![Action::Shell {
                                command: "sudo apt-get update -qq".to_string(),
                            }]),
                    );
                }

                let confirm = requires_confirmation(tool);
                plan.push(
                    Step::new(
                        format!("install:{}", tool.name),
                        StepKind::Install,
                        format!("install {}", tool.name),
                    )
                    .tool(&tool.name)
                    .actions(executor.install(tool)?)
                    .confirm(confirm),
                );
            }
        }

        push_post_install(&mut plan, tool);
    }

    Ok(plan)
}

/// Builds the upgrade plan for tools that a prior upgrade check flagged.
pub fn build_upgrade_plan(tools: &[&Tool], upgrades: &[UpgradeResult]) -> Result<Plan> {
    let by_name: BTreeMap<&str, &UpgradeResult> =
        upgrades.iter().map(|u| (u.tool.as_str(), u)).collect();

    let mut plan = Plan::default();
    let mut apt_update_added = false;

    for tool in tools {
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

        let executor = exec::executor_for(&tool.install);
        if let Some(reason) = executor.blocked_reason(tool) {
            plan.push(
                Step::new(step_id, StepKind::Manual, format!("upgrade {} manually", tool.name))
                    .tool(&tool.name)
                    .blocked(reason),
            );
            continue;
        }

        if matches!(tool.install, Install::Apt { .. }) && !apt_update_added {
            apt_update_added = true;
            plan.push(
                Step::new("apt:update", StepKind::AptUpdate, "refresh apt package lists").actions(
                    vec![Action::Shell {
                        command: "sudo apt-get update -qq".to_string(),
                    }],
                ),
            );
        }

        let latest = result.latest.clone().unwrap_or_else(|| "latest".to_string());
        plan.push(
            Step::new(
                step_id,
                StepKind::Upgrade,
                format!("upgrade {} to {latest}", tool.name),
            )
            .tool(&tool.name)
            .actions(executor.upgrade(tool)?)
            .confirm(requires_confirmation(tool)),
        );
    }

    Ok(plan)
}

fn push_post_install(plan: &mut Plan, tool: &Tool) {
    for (index, post) in tool.post_install.iter().enumerate() {
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

fn requires_confirmation(tool: &Tool) -> bool {
    match &tool.install {
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

    for step in &plan.steps {
        let report = execute_step(step, runner, auth, &mut summary);
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
        let tools = select(&manifest, &Selection::default()).unwrap();
        let plan = build_install_plan(&tools, &probes(&[], &tools)).unwrap();

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
        )
        .unwrap();

        let plan = build_install_plan(&tools, &probes(&["direnv"], &tools)).unwrap();
        let step = plan
            .steps
            .iter()
            .find(|s| s.id == "install:direnv")
            .unwrap();
        assert!(matches!(step.state, StepState::Skipped(_)));
        assert_eq!(plan.pending_count(), 0, "a provisioned machine plans no work");
    }

    #[test]
    fn apt_update_is_emitted_once_before_the_first_apt_install() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(&manifest, &Selection::default()).unwrap();
        let plan = build_install_plan(&tools, &probes(&[], &tools)).unwrap();

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

    #[test]
    fn manual_tools_are_blocked_with_instructions() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(
            &manifest,
            &Selection {
                names: vec!["git-credential-manager".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        let plan = build_install_plan(&tools, &probes(&[], &tools)).unwrap();
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
    fn dry_run_reports_would_run_and_mutates_nothing() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(
            &manifest,
            &Selection {
                names: vec!["curl".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        let plan = build_install_plan(&tools, &probes(&[], &tools)).unwrap();

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
