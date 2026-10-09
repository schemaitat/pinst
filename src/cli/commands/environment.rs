use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{EnvironmentAction, EnvironmentArgs};
use crate::core::plan::{Outcome, Step};
use crate::core::{engine, environment, exec::Runner, home_dir};
use color_eyre::eyre::Result;

pub fn run(ctx: &Ctx, args: &EnvironmentArgs) -> Result<ExitCode> {
    let home = home_dir()?;
    let EnvironmentAction::Build {
        flake,
        configuration,
        out_link,
    } = &args.action;
    let output = out_link
        .clone()
        .unwrap_or_else(|| environment::state_dir(&home).join("build"));
    let plan = environment::build_plan(flake, configuration.as_deref(), &output)?;
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
    let status = if summary.failed > 0 {
        Status::Error
    } else if summary.blocked + summary.needs_confirmation > 0 {
        Status::Issues
    } else {
        Status::Ok
    };
    ctx.finish(
        Envelope::new("environment build", status, reports)
            .dry_run(ctx.dry_run)
            .errors(errors)
            .summary(serde_json::json!({ "execution": summary, "output": output })),
    )
}
