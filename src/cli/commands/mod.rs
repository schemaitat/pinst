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
use crate::core::graph::{self, Selection};
use crate::core::manifest::{LoadedManifest, Tool};

/// Loads the manifest and resolves a selection into dependency-ordered tools.
/// Shared by every tool-facing command so they cannot disagree about
/// selection semantics.
pub fn resolve<'m>(loaded: &'m LoadedManifest, selection: &Selection) -> Result<Vec<&'m Tool>> {
    graph::select(&loaded.manifest, selection)
}

pub fn load_manifest(ctx: &Ctx) -> Result<LoadedManifest> {
    crate::core::manifest::load(ctx.manifest_path.as_ref())
}
