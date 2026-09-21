use color_eyre::eyre::Result;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::core::configs::ConfigSet;
use crate::core::doctor;
use crate::core::engine::{self, Authorizer};
use crate::core::exec::Runner;
use crate::core::plan::{Outcome, Plan, Step, StepReport};
use crate::core::{home_dir, probe};

/// Converges the machine in one command: install what is missing, lay down
/// the configs, then report what is still outstanding. This is the command a
/// cron job or an agent runs unattended to keep a stack in sync.
pub async fn run(ctx: &Ctx, args: &SelectArgs) -> Result<ExitCode> {
    converge(ctx, args, "apply").await
}

/// Shared by `apply` and `bootstrap`: the two differ only in their default
/// selection and their framing, not in what converging means.
pub async fn converge(ctx: &Ctx, args: &SelectArgs, command: &str) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let selected = super::resolve(&loaded, ctx, &args.selection())?;
    let tools = selected.tools;
    let home = home_dir()?;
    let configs = ConfigSet::load(&loaded.manifest, &home)?;

    ctx.note(format!(
        "converging {} tools and {} config files ({}); {} unsupported on {}",
        tools.len(),
        configs.files.len(),
        configs.source.describe(),
        selected.unsupported.len(),
        ctx.platform
    ));

    let probes = probe::probe_all(&tools, ctx.platform).await;

    // Tools first, then configs: a config is only useful once the tool that
    // reads it exists.
    let mut plan = Plan::default();
    plan.steps
        .extend(engine::build_install_plan(&tools, &probes, ctx.platform)?.steps);
    plan.steps.extend(configs.build_plan()?.steps);

    let ctx_for_auth = ctx.clone();
    let approve = move |step: &Step| -> bool {
        ctx_for_auth.confirm(&format!("authorize '{}': {}?", step.id, step.description))
    };
    let runner = Runner::new(ctx.dry_run);
    let quiet = ctx.quiet || ctx.json;
    let (reports, summary) = engine::execute(
        &plan,
        &runner,
        &Authorizer { approve: &approve },
        &mut |report: &StepReport| {
            if quiet || report.outcome == Outcome::Skipped {
                return;
            }
            eprintln!("  {:?} {}", report.outcome, report.description);
        },
    );

    // Re-probe so the closing diagnosis reflects what just happened.
    let probes = probe::probe_all(&tools, ctx.platform).await;
    let mut findings = doctor::diagnose(&tools, &probes, &configs, ctx.platform)?;
    findings.extend(super::unsupported_findings(&selected.unsupported));
    findings.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.id.cmp(&b.id)));
    let doctor_summary = doctor::summarize(&findings);

    if !ctx.json {
        ctx.note(format!(
            "{} ran, {} skipped, {} failed — {} error(s), {} warning(s) remain",
            summary.ran,
            summary.skipped,
            summary.failed,
            doctor_summary.errors,
            doctor_summary.warnings
        ));
        for finding in findings
            .iter()
            .filter(|f| f.severity != doctor::Severity::Info)
        {
            println!(
                "[{:?}] {} — fix: {}",
                finding.severity, finding.message, finding.remediation
            );
        }
    }

    let errors: Vec<String> = reports
        .iter()
        .filter(|r| r.outcome == Outcome::Failed)
        .map(|r| format!("{}: {}", r.id, r.detail.as_deref().unwrap_or("failed")))
        .collect();

    let status = if summary.failed > 0 {
        Status::Error
    } else if doctor_summary.errors > 0 || doctor_summary.warnings > 0 {
        Status::Issues
    } else {
        Status::Ok
    };

    ctx.finish(
        Envelope::new(command, status, reports)
            .dry_run(ctx.dry_run)
            .errors(errors)
            .summary(serde_json::json!({
                "execution": summary,
                "remaining": doctor_summary,
                "findings": findings,
            })),
    )
}
