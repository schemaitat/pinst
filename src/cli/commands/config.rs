use color_eyre::eyre::Result;

use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{ConfigAction, ConfigArgs};
use crate::core::configs::{ConfigSet, FileState};
use crate::core::home_dir;

pub async fn run(ctx: &Ctx, args: &ConfigArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let home = home_dir()?;
    let set = ConfigSet::load(&loaded.manifest, &home)?;
    ctx.note(format!("configs: {}", set.source.describe()));

    match args.action {
        ConfigAction::Status => status(ctx, &set, false),
        ConfigAction::Diff => status(ctx, &set, true),
        ConfigAction::Apply => {
            let plan = set.build_plan()?;
            super::install::execute_and_report(ctx, "config", plan).await
        }
        ConfigAction::Adopt => adopt(ctx, &set),
    }
}

fn status(ctx: &Ctx, set: &ConfigSet, only_differing: bool) -> Result<ExitCode> {
    let all = set.status()?;
    let items: Vec<_> = all
        .into_iter()
        .filter(|s| {
            !only_differing || s.state != FileState::Linked && s.state != FileState::Materialized
        })
        .collect();

    let issues = items
        .iter()
        .filter(|s| !matches!(s.state, FileState::Linked | FileState::Materialized))
        .count();

    if !ctx.json {
        for item in &items {
            let mark = match item.state {
                FileState::Linked => "[ok]",
                FileState::Materialized => "[ok]",
                FileState::Missing => "[--]",
                FileState::Drifted => "[~~]",
                FileState::Foreign => "[!!]",
                FileState::Unrenderable => "[!!]",
            };
            println!("{mark} {:<40} {:?}", item.path, item.state);
        }
    }

    let status = if issues > 0 {
        Status::Issues
    } else {
        Status::Ok
    };
    let total = items.len();
    ctx.finish(
        Envelope::new(
            if only_differing {
                "config diff"
            } else {
                "config status"
            },
            status,
            items,
        )
        .summary(serde_json::json!({ "total": total, "issues": issues })),
    )
}

fn adopt(ctx: &Ctx, set: &ConfigSet) -> Result<ExitCode> {
    let (adopted, skipped) = set.adopt(ctx.dry_run)?;
    if !ctx.json {
        for item in &adopted {
            println!("adopted {}", item.path);
        }
        for reason in &skipped {
            println!("skipped {reason}");
        }
        if adopted.is_empty() && skipped.is_empty() {
            ctx.note("no drifted files to adopt");
        }
    }
    let count = adopted.len();
    // Anything skipped still needs a human, so it is an issue, not success.
    let status = if skipped.is_empty() {
        Status::Ok
    } else {
        Status::Issues
    };
    ctx.finish(
        Envelope::new("config adopt", status, adopted)
            .dry_run(ctx.dry_run)
            .errors(skipped.clone())
            .summary(serde_json::json!({ "adopted": count, "skipped": skipped.len() })),
    )
}
