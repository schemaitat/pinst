use color_eyre::eyre::Result;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::core::engine;
use crate::core::plan::StepState;
use crate::core::probe;

/// Shows the ordered plan without touching anything. Equivalent to
/// `install --dry-run`, but it never executes even a confirmation prompt.
pub async fn run(ctx: &Ctx, args: &SelectArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let selected = super::resolve(&loaded, ctx, &args.selection())?;
    let tools = selected.tools;
    if !ctx.json && !selected.unsupported.is_empty() {
        ctx.note(format!(
            "{} tool(s) skipped: not supported on {}",
            selected.unsupported.len(),
            ctx.platform
        ));
    }

    let probes = probe::probe_all(&tools, ctx.platform).await;
    let plan = engine::build_install_plan(&tools, &probes, ctx.platform)?;

    if !ctx.json {
        for step in &plan.steps {
            match &step.state {
                StepState::Pending => {
                    println!("[ ] {} — {}", step.id, step.description);
                    for action in &step.actions {
                        println!("      $ {}", action.describe());
                    }
                }
                StepState::Skipped(reason) => println!("[x] {} — {reason}", step.id),
                StepState::Blocked(reason) => println!("[!] {} — {reason}", step.id),
            }
        }
    }

    let pending = plan.pending_count();
    let status = if pending > 0 {
        Status::Issues
    } else {
        Status::Ok
    };
    let total = plan.steps.len();
    ctx.finish(
        Envelope::new("plan", status, plan.steps.clone())
            .dry_run(true)
            .summary(serde_json::json!({ "total": total, "pending": pending })),
    )
}
