use color_eyre::eyre::Result;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::core::engine::{self, Authorizer};
use crate::core::exec::Runner;
use crate::core::plan::{Outcome, Plan, Step, StepReport};
use crate::core::probe;

pub async fn run(ctx: &Ctx, args: &SelectArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let tools = super::resolve(&loaded, &args.selection())?;

    ctx.note(format!("probing {} tools...", tools.len()));
    let probes = probe::probe_all(&tools).await;
    let plan = engine::build_install_plan(&tools, &probes)?;

    execute_and_report(ctx, "install", plan).await
}

/// Runs a plan and renders it in both output modes. Shared with `update`,
/// `apply`, and `bootstrap` so every mutating command reports identically.
pub async fn execute_and_report(ctx: &Ctx, command: &str, plan: Plan) -> Result<ExitCode> {
    if plan.pending_count() == 0 {
        ctx.note("nothing to do — everything is already in the desired state");
    }

    let ctx_for_auth = ctx.clone();
    let approve = move |step: &Step| -> bool {
        ctx_for_auth.confirm(&format!(
            "step '{}' ({}) needs authorization: {}. proceed?",
            step.id,
            match step.privilege {
                crate::core::plan::Privilege::Sudo => "sudo",
                crate::core::plan::Privilege::RemoteScript => "remote script",
                crate::core::plan::Privilege::Network => "network",
                crate::core::plan::Privilege::User => "user",
            },
            step.description
        ))
    };

    let runner = Runner::new(ctx.dry_run);
    let auth = Authorizer { approve: &approve };

    let quiet = ctx.quiet || ctx.json;
    let mut on_step = |report: &StepReport| {
        if quiet {
            return;
        }
        let mark = match report.outcome {
            Outcome::Ran => "[ok]",
            Outcome::WouldRun => "[dry]",
            Outcome::Skipped => "[--]",
            Outcome::NeedsConfirmation => "[!!]",
            Outcome::Blocked => "[!!]",
            Outcome::Failed => "[xx]",
        };
        let detail = report
            .detail
            .as_deref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        eprintln!("{mark} {}{detail}", report.description);
        if report.outcome == Outcome::WouldRun {
            for command in &report.commands {
                eprintln!("       $ {command}");
            }
        }
    };

    let (reports, summary) = engine::execute(&plan, &runner, &auth, &mut on_step);

    let errors: Vec<String> = reports
        .iter()
        .filter(|r| r.outcome == Outcome::Failed)
        .map(|r| {
            format!(
                "{}: {}",
                r.id,
                r.detail.as_deref().unwrap_or("failed")
            )
        })
        .collect();

    let status = if summary.failed > 0 {
        Status::Error
    } else if summary.blocked > 0 || summary.needs_confirmation > 0 {
        // Nothing broke, but the operator still has something to do.
        Status::Issues
    } else {
        Status::Ok
    };

    if !ctx.json && !ctx.quiet {
        ctx.note(format!(
            "{} ran, {} skipped, {} failed, {} blocked, {} need authorization, {} would run",
            summary.ran,
            summary.skipped,
            summary.failed,
            summary.blocked,
            summary.needs_confirmation,
            summary.would_run
        ));
    }

    ctx.finish(
        Envelope::new(command, status, reports)
            .dry_run(ctx.dry_run)
            .errors(errors)
            .summary(serde_json::to_value(&summary)?),
    )
}
