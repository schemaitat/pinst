use color_eyre::eyre::Result;

use crate::cli::output::{Ctx, ExitCode};
use crate::cli::{SchemaArgs, SchemaKind};
use crate::core::docs::page::ToolDoc;
use crate::core::manifest::Manifest;

/// Emits the JSON Schema derived from the same Rust types the loader uses, so
/// an agent can author or validate a manifest without reading source and
/// without the schema drifting from the parser.
pub fn run(ctx: &Ctx, args: &SchemaArgs) -> Result<ExitCode> {
    let schema = match args.kind {
        SchemaKind::Manifest => schemars::schema_for!(Manifest),
        SchemaKind::Output => schemars::schema_for!(OutputEnvelopeSchema),
        SchemaKind::Docs => schemars::schema_for!(ToolDoc),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema is what an agent authors a page against, so it has to know
    /// every key a real page uses. Without a JSON Schema validator in the
    /// dependency tree — and one is not worth adding to a binary tuned for
    /// size — checking the real page's keys against the emitted properties is
    /// the drift this can actually catch.
    #[test]
    fn the_docs_schema_covers_every_key_the_authored_page_uses() {
        let schema = serde_json::to_value(schemars::schema_for!(ToolDoc)).unwrap();
        let properties = schema["properties"].as_object().unwrap();

        let page: toml::Value = toml::from_str(include_str!("../../../docs/tools/ripgrep.toml"))
            .expect("the authored page must parse as TOML");
        for key in page.as_table().unwrap().keys() {
            assert!(
                properties.contains_key(key.as_str()),
                "docs page key `{key}` is missing from `pinst schema docs`"
            );
        }

        assert_eq!(
            schema["required"],
            serde_json::json!(["what"]),
            "`what` is the one field a page cannot omit"
        );
    }
}
