use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{EnvironmentAction, EnvironmentArgs};
use crate::core::plan::{Outcome, Step};
use crate::core::{engine, environment, exec::Runner, home_dir};
use color_eyre::eyre::Result;

pub fn run(ctx: &Ctx, args: &EnvironmentArgs) -> Result<ExitCode> {
    let home = home_dir()?;
    let output = environment::state_dir(&home).join("build");
    let (name, plan, output) = match &args.action {
        EnvironmentAction::Build {
            flake,
            configuration,
            out_link,
        } => {
            let output = out_link.clone().unwrap_or(output);
            (
                "environment build",
                environment::build_plan(flake, configuration.as_deref(), &output)?,
                output,
            )
        }
        EnvironmentAction::Apply {
            generation,
            migrate,
        } => {
            let output = generation.clone().unwrap_or(output);
            (
                "environment apply",
                environment::activation_plan(&home, &output, *migrate)?,
                output,
            )
        }
        EnvironmentAction::Rollback => (
            "environment rollback",
            environment::rollback_plan(&home)?,
            environment::state_dir(&home).join("previous"),
        ),
        EnvironmentAction::Status => {
            let (items, healthy) = environment::status(&home)?;
            for item in &items {
                ctx.note(item);
            }
            return ctx.finish(Envelope::new("environment status", if healthy { Status::Ok } else { Status::Issues }, items)
                .summary(serde_json::json!({ "managed_healthy": healthy, "external_tools_are_unmanaged": true })));
        }
    };
    let approve = |step: &Step| ctx.confirm(&step.description);
    let (reports, summary) = engine::execute(
        &plan,
        &Runner::new(ctx.dry_run),
        &engine::Authorizer { approve: &approve },
        &mut |report| {
            ctx.note(format!("{:?}: {}", report.outcome, report.description));
            for command in &report.commands {
                ctx.note(command);
            }
        },
    );
    let errors = reports
        .iter()
        .filter(|r| r.outcome == Outcome::Failed)
        .map(|r| r.detail.clone().unwrap_or_default())
        .collect();
    let verified = if !ctx.dry_run
        && summary.failed + summary.blocked + summary.needs_confirmation == 0
        && name != "environment build"
    {
        Some(environment::status(&home)?.1)
    } else {
        None
    };
    let status = if summary.failed > 0 {
        Status::Error
    } else if summary.blocked + summary.needs_confirmation > 0 || verified == Some(false) {
        Status::Issues
    } else {
        Status::Ok
    };
    ctx.finish(
        Envelope::new(name, status, reports)
            .dry_run(ctx.dry_run)
            .errors(errors)
            .summary(serde_json::json!({ "execution": summary, "output": output, "managed_healthy": verified })),
    )
}
