use color_eyre::eyre::Result;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, ExitCode};
use crate::core::manifest::Tool;
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
    // The lookups shell out and hit the network, so they belong on the
    // blocking pool rather than the async runtime's worker threads.
    let owned: Vec<Tool> = tools.iter().map(|t| (*t).clone()).collect();
    let platform = ctx.platform;
    let upgrades = tokio::task::spawn_blocking(move || {
        let refs: Vec<&Tool> = owned.iter().collect();
        upgrade::check_all(&refs, &probes, false, platform)
    })
    .await?;

    let plan = engine::build_upgrade_plan(&tools, &upgrades, ctx.platform)?;
    super::install::execute_and_report(ctx, "update", plan).await
}
