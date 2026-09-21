pub mod apply;
pub mod bootstrap;
pub mod config;
pub mod docs;
pub mod doctor;
pub mod harness;
pub mod install;
pub mod list;
pub mod plan;
pub mod schema;
pub mod tui;
pub mod update;

use color_eyre::eyre::Result;

use crate::cli::output::Ctx;
use crate::core::doctor::{Finding, unsupported_finding};
use crate::core::graph::{self, Selected, Selection, UnsupportedTool};
use crate::core::manifest::LoadedManifest;

/// Loads the manifest and resolves a selection into dependency-ordered tools
/// for `ctx.platform`. Shared by every tool-facing command so they cannot
/// disagree about selection semantics.
pub fn resolve<'m>(
    loaded: &'m LoadedManifest,
    ctx: &Ctx,
    selection: &Selection,
) -> Result<Selected<'m>> {
    graph::select(&loaded.manifest, selection, ctx.platform)
}

/// The `tool.unsupported.<name>` finding for every tool `resolve` dropped —
/// reportable the same way `doctor::diagnose`'s other findings are, so a
/// command that narrows by platform never just goes quiet about the tools it
/// narrowed away.
pub fn unsupported_findings(unsupported: &[UnsupportedTool]) -> Vec<Finding> {
    unsupported
        .iter()
        .map(|u| unsupported_finding(&u.name, &u.note))
        .collect()
}

pub fn load_manifest(ctx: &Ctx) -> Result<LoadedManifest> {
    crate::core::manifest::load(ctx.manifest_path.as_ref())
}
