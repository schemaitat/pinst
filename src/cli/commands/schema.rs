use color_eyre::eyre::Result;

use crate::cli::output::{Ctx, ExitCode};
use crate::cli::{SchemaArgs, SchemaKind};
use crate::core::manifest::Manifest;

/// Emits the JSON Schema derived from the same Rust types the loader uses, so
/// an agent can author or validate a manifest without reading source and
/// without the schema drifting from the parser.
pub fn run(ctx: &Ctx, args: &SchemaArgs) -> Result<ExitCode> {
    let schema = match args.kind {
        SchemaKind::Manifest => schemars::schema_for!(Manifest),
        SchemaKind::Output => schemars::schema_for!(OutputEnvelopeSchema),
    };
    // The schema is the payload, so it goes to stdout in both modes rather
    // than being wrapped in the envelope.
    println!("{}", serde_json::to_string_pretty(&schema)?);
    let _ = ctx;
    Ok(ExitCode::Success)
}

/// Mirrors `output::Envelope` for schema generation. `Envelope<T>` is generic
/// over the per-command item type, so this documents the stable outer shape
/// with items left open.
#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct OutputEnvelopeSchema {
    /// Bumped only on a breaking change to this envelope.
    schema_version: u32,
    /// The command that produced this envelope.
    command: String,
    /// One of: ok, issues, error.
    status: String,
    /// True when --dry-run suppressed all mutation.
    dry_run: bool,
    /// Command-specific payload.
    items: Vec<serde_json::Value>,
    /// Human-readable failure messages.
    errors: Vec<String>,
    /// Command-specific counts.
    summary: Option<serde_json::Value>,
}
