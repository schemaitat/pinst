use color_eyre::eyre::Result;

use crate::cli::DoctorArgs;
use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::core::configs::{ConfigSet, FileState};
use crate::core::doctor::{self, Finding, Severity};
use crate::core::engine::{self, Authorizer};
use crate::core::exec::Runner;
use crate::core::graph::Selection;
use crate::core::manifest::Tool;
use crate::core::plan::{Plan, Step};
use crate::core::{home_dir, probe};

pub async fn run(ctx: &Ctx, args: &DoctorArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let tools = super::resolve(&loaded, &Selection::default())?;
    let home = home_dir()?;
    let configs = ConfigSet::load(&loaded.manifest, &home)?;

    ctx.note(format!("configs: {}", configs.source.describe()));
    let probes = probe::probe_all(&tools).await;
    let findings = doctor::diagnose(&tools, &probes, &configs)?;

    if args.fix {
        return fix(ctx, &tools, &probes, &configs, findings).await;
    }

    report(ctx, "doctor", findings, false)
}

fn report(
    ctx: &Ctx,
    command: &str,
    findings: Vec<Finding>,
    fixed_something: bool,
) -> Result<ExitCode> {
    if !ctx.json {
        for finding in &findings {
            let mark = match finding.severity {
                Severity::Error => "[error]",
                Severity::Warning => "[warn ]",
                Severity::Info => "[info ]",
            };
            println!("{mark} {}", finding.message);
            println!("         fix: {}", finding.remediation);
        }
        if findings.is_empty() {
            println!("everything checks out");
        }
    }

    let summary = doctor::summarize(&findings);
    // Warnings and errors both mean "there is something to act on", which is
    // the distinction exit code 3 exists to carry.
    let status = if summary.errors > 0 || summary.warnings > 0 {
        Status::Issues
    } else {
        Status::Ok
    };

    let mut value = serde_json::to_value(&summary)?;
    if let Some(map) = value.as_object_mut() {
        map.insert("fixed".to_string(), serde_json::Value::Bool(fixed_something));
    }

    ctx.finish(
        Envelope::new(command, status, findings)
            .dry_run(ctx.dry_run)
            .summary(value),
    )
}

/// Applies only the safe subset: install missing tools, and put missing or
/// foreign configs right. Drifted files are left alone — see the finding's
/// remediation for why that is a human decision.
async fn fix(
    ctx: &Ctx,
    tools: &[&Tool],
    probes: &std::collections::BTreeMap<String, crate::core::probe::ProbeResult>,
    configs: &ConfigSet,
    findings: Vec<Finding>,
) -> Result<ExitCode> {
    let fixable: Vec<&Finding> = findings.iter().filter(|f| f.fixable).collect();
    if fixable.is_empty() {
        ctx.note("nothing fixable — remaining findings need a human decision");
        return report(ctx, "doctor", findings, false);
    }
    ctx.note(format!("fixing {} finding(s)...", fixable.len()));

    let missing: Vec<&Tool> = tools
        .iter()
        .copied()
        .filter(|tool| {
            findings
                .iter()
                .any(|f| f.fixable && f.id == format!("tool.missing.{}", tool.name))
        })
        .collect();

    let mut plan = Plan::default();
    if !missing.is_empty() {
        let install_plan = engine::build_install_plan(&missing, probes)?;
        plan.steps.extend(install_plan.steps);
    }
    let config_plan = configs.build_plan_where(|state| {
        matches!(state, FileState::Missing | FileState::Foreign)
    })?;
    plan.steps.extend(config_plan.steps);

    let ctx_for_auth = ctx.clone();
    let approve = move |step: &Step| -> bool {
        ctx_for_auth.confirm(&format!("authorize '{}': {}?", step.id, step.description))
    };
    let runner = Runner::new(ctx.dry_run);
    let (_, summary) = engine::execute(
        &plan,
        &runner,
        &Authorizer { approve: &approve },
        &mut |report| {
            if !ctx.json && !ctx.quiet {
                eprintln!("  {:?} {}", report.outcome, report.description);
            }
        },
    );
    ctx.note(format!(
        "{} applied, {} failed, {} need authorization",
        summary.ran, summary.failed, summary.needs_confirmation
    ));

    // Re-diagnose so the report reflects reality after the fixes, not before.
    let probes = probe::probe_all(tools).await;
    let remaining = doctor::diagnose(tools, &probes, configs)?;
    report(ctx, "doctor", remaining, summary.ran > 0)
}
