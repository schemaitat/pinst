use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
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
    let tools = super::resolve(&loaded, &args.selection())?;

    ctx.note(format!(
        "manifest: {} ({} tools selected)",
        loaded.source,
        tools.len()
    ));

    let probes = probe::probe_all(&tools).await;

    let items: Vec<ToolStatus> = tools
        .iter()
        .map(|tool| {
            let probe = probes.get(&tool.name);
            ToolStatus {
                name: tool.name.clone(),
                summary: tool.summary.clone(),
                tags: tool.tags.clone(),
                requires: tool.requires.clone(),
                install_method: tool.method_name().to_string(),
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
