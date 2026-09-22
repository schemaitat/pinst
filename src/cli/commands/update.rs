use color_eyre::eyre::Result;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, ExitCode};
use crate::core::{engine, probe, upgrade};

pub async fn run(ctx: &Ctx, args: &SelectArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let selected = super::resolve(&loaded, ctx, &args.selection())?;
    let tools = selected.tools;
    if !selected.unsupported.is_empty() {
        ctx.note(format!(
            "{} tool(s) skipped: not supported on {}",
            selected.unsupported.len(),
            ctx.platform
        ));
    }

    ctx.note(format!("probing {} tools...", tools.len()));
    let probes = probe::probe_all(&tools, ctx.platform).await;

    ctx.note("checking for newer versions...");
    let upgrades = upgrade::check_all(&tools, &probes, false, ctx.platform).await;

    let plan = engine::build_upgrade_plan(&tools, &upgrades, ctx.platform)?;
    super::install::execute_and_report(ctx, "update", plan).await
}
