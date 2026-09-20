//! `pinst docs` — the tool catalogue, for a caller who has a task and does
//! not yet know which tool does it.
//!
//! The exit codes carry the verdict here as everywhere else: an unknown tool
//! is a usage error (2), a tool the manifest declares but nothing documents is
//! something to act on (3), and a page is a success (0). Coverage is
//! deliberately *not* in that list — `status` always exits 0, because an
//! unwritten page is a normal state and a check that fires on the normal state
//! is a check that gets switched off.

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{DocsAction, DocsArgs, DocsShowArgs};
use crate::core::docs::Catalogue;
use crate::core::docs::page::ToolDoc;
use crate::core::manifest::Tool;
use crate::core::{probe, usage};

/// Where the answer came from. An authored page and a dump of the tool's own
/// `--help` have very different reliability, so they are never collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocSource {
    /// A page under `docs/tools/`.
    Page,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShowItem {
    pub source: DocSource,
    pub installed: bool,
    pub version: Option<String>,
    #[serde(flatten)]
    pub page: ToolDoc,
}

/// What `status` reports per tool. `page` is the coverage verdict; everything
/// else is the context needed to decide whether the gap matters.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CoverageItem {
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
    /// `authored`, `draft`, or `none`.
    pub page: String,
    pub installed: bool,
    pub version: Option<String>,
    /// The version the page's recipes were verified against, when it says.
    pub verified_with: Option<String>,
    /// True when the page was verified against a different version than the
    /// one installed. Reported, never enforced.
    pub stale: bool,
}

pub async fn run(ctx: &Ctx, args: &DocsArgs) -> Result<ExitCode> {
    let loaded = super::load_manifest(ctx)?;
    let catalogue = Catalogue::load()?;
    if catalogue.is_empty() {
        ctx.note(format!(
            "docs: {} (no pages yet)",
            catalogue.source.describe()
        ));
    } else {
        ctx.note(format!(
            "docs: {} ({} pages)",
            catalogue.source.describe(),
            catalogue.len()
        ));
    }

    match &args.action {
        DocsAction::Show(show) => self::show(ctx, &loaded.manifest, &catalogue, show).await,
        DocsAction::Status => status(ctx, &loaded.manifest, &catalogue).await,
    }
}

async fn show(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
    args: &DocsShowArgs,
) -> Result<ExitCode> {
    let tool = lookup(manifest, &args.tool)?;
    let probe = probe::probe_tool(tool).await;

    let Some(page) = catalogue.get(&tool.name) else {
        // Nothing failed: the command ran, and found something to act on.
        if !ctx.json {
            println!("{}  {}", tool.name, tool.summary);
            println!("no page yet — {}", remediation(&tool.name));
        }
        return ctx.finish(
            Envelope::<ShowItem>::new("docs show", Status::Issues, Vec::new()).errors(vec![
                format!("{}: no docs page. {}", tool.name, remediation(&tool.name)),
            ]),
        );
    };

    if !ctx.json {
        render(page, &probe.version);
    }

    ctx.finish(Envelope::new(
        "docs show",
        Status::Ok,
        vec![ShowItem {
            source: DocSource::Page,
            installed: probe.installed,
            version: probe.version,
            page: page.clone(),
        }],
    ))
}

async fn status(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
) -> Result<ExitCode> {
    let tools: Vec<&Tool> = manifest.tools.iter().collect();
    let probes = probe::probe_all(&tools).await;
    let (items, orphans) = coverage(manifest, catalogue, &probes);

    let authored = items.iter().filter(|i| i.page == "authored").count();
    let draft = items.iter().filter(|i| i.page == "draft").count();
    let missing = items.iter().filter(|i| i.page == "none").count();
    let stale = items.iter().filter(|i| i.stale).count();

    if !ctx.json {
        for item in &items {
            let mark = match item.page.as_str() {
                "authored" => "[ok]",
                "draft" => "[~~]",
                _ => "[--]",
            };
            let note = if item.stale {
                format!(
                    " (verified with {})",
                    item.verified_with.as_deref().unwrap_or("?")
                )
            } else {
                String::new()
            };
            println!("{mark} {:<24} {:<10}{}", item.name, item.page, note);
        }
        for orphan in &orphans {
            println!("[!!] {orphan:<24} orphan     (no such tool in the manifest)");
        }
    }

    let total = items.len();
    // Always Ok: coverage is a report. A gate that is red by design for the
    // whole life of a catalogue is a gate people learn to skip.
    ctx.finish(
        Envelope::new("docs status", Status::Ok, items).summary(serde_json::json!({
            "total": total,
            "authored": authored,
            "draft": draft,
            "missing": missing,
            "stale": stale,
            "orphans": orphans,
        })),
    )
}

/// One row per manifest tool, plus the pages that name no tool at all.
///
/// Split out from `status` because it is the whole judgement — what counts as
/// covered, and what counts as stale — and it is worth testing without a
/// terminal or a subprocess in the way.
fn coverage(
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
    probes: &std::collections::BTreeMap<String, probe::ProbeResult>,
) -> (Vec<CoverageItem>, Vec<String>) {
    let items: Vec<CoverageItem> = manifest
        .tools
        .iter()
        .map(|tool| {
            let page = catalogue.get(&tool.name);
            let probe = probes.get(&tool.name);
            let version = probe.and_then(|p| p.version.clone());
            let verified_with = page.and_then(|p| p.verified_with.clone());
            // Only a page that *claims* a version can be stale against the
            // installed one; silence is not a claim.
            let stale = match (&verified_with, &version) {
                (Some(verified), Some(installed)) => verified != installed,
                _ => false,
            };
            CoverageItem {
                name: tool.name.clone(),
                summary: tool.summary.clone(),
                tags: tool.tags.clone(),
                page: match page {
                    Some(page) => page.status.as_str().to_string(),
                    None => "none".to_string(),
                },
                installed: probe.map(|p| p.installed).unwrap_or(false),
                version,
                verified_with,
                stale,
            }
        })
        .collect();

    // A page for a tool the manifest no longer declares. `cargo test` fails on
    // one in the embedded catalogue, but a checkout mid-rename can have them,
    // and reporting beats silently skipping.
    let orphans: Vec<String> = catalogue
        .iter()
        .map(|page| page.name.clone())
        .filter(|name| !manifest.tools.iter().any(|tool| &tool.name == name))
        .collect();

    (items, orphans)
}

/// The manifest is the scope: a name it does not declare is a bad invocation,
/// not an empty result.
fn lookup<'m>(manifest: &'m crate::core::manifest::Manifest, name: &str) -> Result<&'m Tool> {
    manifest
        .tools
        .iter()
        .find(|tool| tool.name == name)
        .ok_or_else(|| usage(format!("unknown tool '{name}'")))
}

fn remediation(tool: &str) -> String {
    format!("write docs/tools/{tool}.toml")
}

/// The human rendering: intent first, then the command lines, because that is
/// the order the reader needs them in.
fn render(page: &ToolDoc, installed_version: &Option<String>) {
    let mut badge = page.status.as_str().to_string();
    if let Some(verified) = &page.verified_with {
        badge.push_str(&format!(", verified with {verified}"));
    }
    if let (Some(verified), Some(installed)) = (&page.verified_with, installed_version)
        && verified != installed
    {
        badge.push_str(&format!(", installed {installed}"));
    }
    println!("{}  [{}]", page.name, badge);
    println!("{}", page.what);

    if let Some(when) = &page.when {
        println!("\nWhen: {when}");
    }

    if !page.recipes.is_empty() {
        println!();
        for recipe in &page.recipes {
            println!("  $ {}", recipe.cmd);
            println!("      {}", recipe.does);
        }
    }

    if let Some(gotchas) = &page.gotchas {
        println!("\nGotchas:");
        for line in gotchas.trim().lines() {
            println!("  {line}");
        }
    }

    if !page.see_also.is_empty() {
        println!("\nSee also: {}", page.see_also.join(", "));
    }

    if let Some(help) = &page.help {
        println!("\nCaptured help:");
        for line in help.trim().lines() {
            println!("  {line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UsageError;
    use crate::core::manifest::Manifest;
    use crate::core::source::Source;

    const MANIFEST: &str = r#"
[meta]
schema_version = 1

[[tool]]
name = "ripgrep"
summary = "Fast grep"
tags = ["dev"]
detect = { command = "true", bin = "rg" }
install = { method = "apt", packages = ["ripgrep"] }

[[tool]]
name = "fd"
summary = "Fast find"
detect = { command = "false" }
install = { method = "apt", packages = ["fd-find"] }
"#;

    fn manifest() -> Manifest {
        toml::from_str(MANIFEST).unwrap()
    }

    /// The tempdir is returned alongside: dropping it would delete the pages
    /// out from under the catalogue.
    fn catalogue(pages: &[(&str, &str)]) -> (tempfile::TempDir, Catalogue) {
        let dir = tempfile::tempdir().unwrap();
        for (name, body) in pages {
            std::fs::write(dir.path().join(format!("{name}.toml")), body).unwrap();
        }
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        (dir, catalogue)
    }

    fn ctx() -> Ctx {
        Ctx {
            json: false,
            dry_run: false,
            yes: false,
            quiet: true,
            manifest_path: None,
        }
    }

    fn show_args(tool: &str) -> DocsShowArgs {
        DocsShowArgs {
            tool: tool.to_string(),
        }
    }

    #[tokio::test]
    async fn a_tool_with_a_page_exits_zero() {
        let (_dir, catalogue) = catalogue(&[("ripgrep", r#"what = "Fast grep.""#)]);
        let code = show(&ctx(), &manifest(), &catalogue, &show_args("ripgrep"))
            .await
            .unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[tokio::test]
    async fn a_manifest_tool_with_no_page_is_something_to_act_on_not_a_failure() {
        let (_dir, catalogue) = catalogue(&[]);
        let code = show(&ctx(), &manifest(), &catalogue, &show_args("fd"))
            .await
            .unwrap();
        assert_eq!(code, ExitCode::Issues);
    }

    #[tokio::test]
    async fn a_name_the_manifest_does_not_declare_is_a_usage_error() {
        let (_dir, catalogue) = catalogue(&[]);
        let err = show(&ctx(), &manifest(), &catalogue, &show_args("cowsay"))
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<UsageError>().is_some(),
            "an unknown tool must map to exit 2, not 1: {err:#}"
        );
    }

    #[tokio::test]
    async fn the_remediation_names_the_file_to_write() {
        let (_dir, catalogue) = catalogue(&[]);
        // The exit-3 path is only useful if it says what to do about it; the
        // envelope carries this same string in `errors[]`.
        assert_eq!(remediation("fd"), "write docs/tools/fd.toml");
        let code = show(&ctx(), &manifest(), &catalogue, &show_args("fd"))
            .await
            .unwrap();
        assert_eq!(code, ExitCode::Issues);
    }

    #[tokio::test]
    async fn status_reports_an_empty_catalogue_without_failing() {
        let (_dir, catalogue) = catalogue(&[]);
        let code = status(&ctx(), &manifest(), &catalogue).await.unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[tokio::test]
    async fn status_still_exits_zero_with_a_full_catalogue() {
        let (_dir, catalogue) = catalogue(&[
            ("ripgrep", r#"what = "Fast grep.""#),
            ("fd", r#"what = "Fast find.""#),
        ]);
        let code = status(&ctx(), &manifest(), &catalogue).await.unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[test]
    fn coverage_classifies_every_tool_and_counts_the_gaps() {
        let (_dir, catalogue) = catalogue(&[
            ("ripgrep", "what = \"Fast grep.\"\nstatus = \"authored\"\n"),
            ("fd", r#"what = "Fast find.""#),
        ]);
        let (items, orphans) = coverage(&manifest(), &catalogue, &Default::default());

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "ripgrep");
        assert_eq!(items[0].page, "authored");
        assert_eq!(items[1].page, "draft", "an unmarked page is a draft");
        assert!(orphans.is_empty());
    }

    #[test]
    fn a_tool_with_no_page_is_reported_as_none() {
        let (_dir, catalogue) = catalogue(&[("ripgrep", r#"what = "Fast grep.""#)]);
        let (items, _) = coverage(&manifest(), &catalogue, &Default::default());
        assert_eq!(items[1].page, "none");
        assert!(!items[1].stale);
    }

    #[test]
    fn a_page_for_a_tool_the_manifest_dropped_is_an_orphan() {
        let (_dir, catalogue) = catalogue(&[("cowsay", r#"what = "Moo.""#)]);
        let (_, orphans) = coverage(&manifest(), &catalogue, &Default::default());
        assert_eq!(orphans, vec!["cowsay".to_string()]);
    }

    #[test]
    fn a_page_verified_against_another_version_is_stale_but_still_a_page() {
        let (_dir, catalogue) = catalogue(&[(
            "ripgrep",
            "what = \"Fast grep.\"\nstatus = \"authored\"\nverified_with = \"14.0.0\"\n",
        )]);
        let mut probes = std::collections::BTreeMap::new();
        probes.insert(
            "ripgrep".to_string(),
            probe::ProbeResult {
                tool: "ripgrep".to_string(),
                installed: true,
                version: Some("15.1.0".to_string()),
                path: None,
            },
        );

        let (items, _) = coverage(&manifest(), &catalogue, &probes);
        assert!(items[0].stale);
        assert_eq!(items[0].page, "authored");
    }

    #[test]
    fn a_page_that_claims_no_version_is_never_stale() {
        // Silence is not a claim — otherwise every unversioned page would be
        // permanently reported as drifted.
        let (_dir, catalogue) = catalogue(&[("ripgrep", r#"what = "Fast grep.""#)]);
        let mut probes = std::collections::BTreeMap::new();
        probes.insert(
            "ripgrep".to_string(),
            probe::ProbeResult {
                tool: "ripgrep".to_string(),
                installed: true,
                version: Some("15.1.0".to_string()),
                path: None,
            },
        );
        let (items, _) = coverage(&manifest(), &catalogue, &probes);
        assert!(!items[0].stale);
    }
}
