use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::core::manifest::Resolved;
use crate::core::probe;

#[derive(Debug, Serialize, JsonSchema)]
pub struct ToolStatus {
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub requires: Vec<String>,
    pub install_method: String,
    pub installed: bool,
    pub version: Option<String>,
}

pub async fn run(ctx: &Ctx, args: &SelectArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let selected = super::resolve(&loaded, ctx, &args.selection())?;
    let tools = selected.tools;

    ctx.note(format!(
        "manifest: {} ({} tools selected, {} unsupported on {})",
        loaded.source,
        tools.len(),
        selected.unsupported.len(),
        ctx.platform
    ));

    let probes = probe::probe_all(&tools, ctx.platform).await;

    let items: Vec<ToolStatus> = tools
        .iter()
        .map(|tool| {
            let probe = probes.get(&tool.name);
            // `tools` is already filtered to what `ctx.platform` supports, so
            // this is always `Supported`; the base method name is a harmless
            // fallback for the one caller path (none today) that might hand
            // in an unfiltered tool.
            let install_method = match tool.resolve(ctx.platform) {
                Resolved::Supported(effective) => effective.method_name().to_string(),
                Resolved::Unsupported(_) => tool.method_name().to_string(),
            };
            ToolStatus {
                name: tool.name.clone(),
                summary: tool.summary.clone(),
                tags: tool.tags.clone(),
                requires: tool.requires.clone(),
                install_method,
                installed: probe.map(|p| p.installed).unwrap_or(false),
                version: probe.and_then(|p| p.version.clone()),
            }
        })
        .collect();

    let missing = items.iter().filter(|i| !i.installed).count();

    if !ctx.json {
        for item in &items {
            let mark = if item.installed { "[ok]" } else { "[--]" };
            let version = item.version.as_deref().unwrap_or("-");
            println!("{mark} {:<24} {:<10} {}", item.name, version, item.summary);
        }
    }

    // Missing tools are a reportable condition, not a failure of `list`
    // itself — the exit code tells an agent there is work to do.
    let status = if missing > 0 {
        Status::Issues
    } else {
        Status::Ok
    };
    let total = items.len();
    ctx.finish(
        Envelope::new("list", status, items)
            .summary(serde_json::json!({ "total": total, "missing": missing })),
    )
}
