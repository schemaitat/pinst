//! `pinst docs` — the tool catalogue, for a caller who has a task and does
//! not yet know which tool does it.
//!
//! The exit codes carry the verdict here as everywhere else: an unknown tool
//! is a usage error (2), a tool the manifest declares but nothing documents is
//! something to act on (3), and a page is a success (0). Coverage is
//! deliberately *not* in that list — `status` always exits 0, because an
//! unwritten page is a normal state and a check that fires on the normal state
//! is a check that gets switched off.

use color_eyre::eyre::{Context, Result};
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{DocsAction, DocsAdoptArgs, DocsArgs, DocsDumpArgs, DocsSearchArgs, DocsShowArgs};
use crate::core::docs::page::ToolDoc;
use crate::core::docs::{Catalogue, capture, search as find, seed};
use crate::core::graph::{self, Selection};
use crate::core::manifest::Tool;
use crate::core::platform::Platform;
use crate::core::{probe, usage};

/// Where the answer came from. An authored page and a dump of the tool's own
/// `--help` have very different reliability, so they are never collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocSource {
    /// A page under `docs/tools/`.
    Page,
    /// The tool's own `--help`, captured from this machine. Written for
    /// someone who already knows they want this tool — worth answering with,
    /// worth trusting less.
    Captured,
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
        DocsAction::Search(search) => self::search(ctx, &loaded.manifest, &catalogue, search).await,
        DocsAction::Dump(dump) => self::dump(ctx, &loaded.manifest, &catalogue, dump),
        // The cache location is resolved once, here, and passed down: it is
        // ambient state otherwise, and ambient state is what makes a test
        // reach into the developer's real home directory.
        DocsAction::Show(show) => {
            self::show(
                ctx,
                &loaded.manifest,
                &catalogue,
                &capture::cache_dir()?,
                show,
            )
            .await
        }
        DocsAction::Status => status(ctx, &loaded.manifest, &catalogue).await,
        DocsAction::Adopt(adopt) => {
            self::adopt(
                ctx,
                &loaded.manifest,
                &catalogue,
                &capture::cache_dir()?,
                adopt,
            )
            .await
        }
    }
}

async fn show(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
    cache_dir: &std::path::Path,
    args: &DocsShowArgs,
) -> Result<ExitCode> {
    let tool = lookup(manifest, &args.tool)?;
    let probe = probe::probe_tool(tool, Platform::host()).await;

    // An authored page always wins: it was written for this question, and the
    // capture was not.
    if let Some(page) = catalogue.get(&tool.name) {
        if !ctx.json {
            render(page, &probe.version);
        }
        return ctx.finish(Envelope::new(
            "docs show",
            Status::Ok,
            vec![ShowItem {
                source: DocSource::Page,
                installed: probe.installed,
                version: probe.version,
                page: page.clone(),
            }],
        ));
    }

    let captured =
        capture::capture(tool, probe.version.as_deref(), args.refresh, cache_dir).await?;

    let Some(captured) = captured else {
        // Nothing failed: the command ran, and found something to act on.
        if !ctx.json {
            println!("{}  {}", tool.name, tool.summary);
            println!("no page yet — {}", remediation(tool));
        }
        return ctx.finish(
            Envelope::<ShowItem>::new("docs show", Status::Issues, Vec::new()).errors(vec![
                format!("{}: no docs page. {}", tool.name, remediation(tool)),
            ]),
        );
    };

    let page = seed::synthesize(tool, &captured);
    if !ctx.json {
        ctx.note(format!(
            "no page for {} — showing `{}`; {}",
            tool.name,
            captured.command,
            remediation(tool)
        ));
        render(&page, &probe.version);
    }

    // Exit 0: the question was answered. That it was answered by the tool
    // rather than by a page is what `source` is for.
    ctx.finish(Envelope::new(
        "docs show",
        Status::Ok,
        vec![ShowItem {
            source: DocSource::Captured,
            installed: probe.installed,
            version: probe.version,
            page,
        }],
    ))
}

/// What `adopt` did, or would have done under `--dry-run`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AdoptItem {
    pub tool: String,
    pub path: String,
    /// The help command the seed came from.
    pub command: String,
    pub bytes: usize,
    /// True when `--dry-run` meant nothing was written.
    pub planned: bool,
}

async fn adopt(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
    cache_dir: &std::path::Path,
    args: &DocsAdoptArgs,
) -> Result<ExitCode> {
    let tool = lookup(manifest, &args.tool)?;

    // Refusals first, before running anything: a command that cannot write
    // should not spend a subprocess finding that out.
    let Some(path) = catalogue.page_path(&tool.name) else {
        return refuse(
            ctx,
            format!(
                "{}: the catalogue is embedded in the binary, so there is no tree to write into. \
                 Run adopt from a checkout of this repo.",
                tool.name
            ),
        );
    };

    if let Some(existing) = catalogue.get(&tool.name) {
        if existing.status == crate::core::docs::page::PageStatus::Authored {
            return refuse(
                ctx,
                format!(
                    "{}: docs/tools/{}.toml is authored; adopt would replace written prose with \
                     captured help. Edit the page instead.",
                    tool.name, tool.name
                ),
            );
        }
        if !ctx.yes && !ctx.dry_run {
            return refuse(
                ctx,
                format!(
                    "{}: docs/tools/{}.toml already exists as a draft. Re-seed it with --yes.",
                    tool.name, tool.name
                ),
            );
        }
    }

    let probe = probe::probe_tool(tool, Platform::host()).await;
    let captured =
        capture::capture(tool, probe.version.as_deref(), args.refresh, cache_dir).await?;

    let Some(captured) = captured else {
        return refuse(
            ctx,
            format!(
                "{}: nothing to adopt — {}. {}",
                tool.name,
                match capture::help_command(tool) {
                    Some(command) => format!("`{command}` produced no output"),
                    None => "the manifest declares no help command and no bin".to_string(),
                },
                remediation(tool)
            ),
        );
    };

    let body = seed::render(tool, &captured);
    if !ctx.dry_run {
        std::fs::write(&path, &body).with_context(|| format!("writing {}", path.display()))?;
    }

    if !ctx.json {
        if ctx.dry_run {
            println!("would write {} ({} bytes)", path.display(), body.len());
        } else {
            println!("wrote {} ({} bytes)", path.display(), body.len());
            println!("it is a draft: read it, add recipes you have run, then mark it authored");
        }
    }

    ctx.finish(
        Envelope::new(
            "docs adopt",
            Status::Ok,
            vec![AdoptItem {
                tool: tool.name.clone(),
                path: path.display().to_string(),
                command: captured.command,
                bytes: body.len(),
                planned: ctx.dry_run,
            }],
        )
        .dry_run(ctx.dry_run),
    )
}

/// A refusal is something to act on, not a failure of the command: exit 3,
/// with the reason in `errors[]` where an agent already looks.
fn refuse(ctx: &Ctx, reason: String) -> Result<ExitCode> {
    if !ctx.json {
        println!("{reason}");
    }
    ctx.finish(
        Envelope::<AdoptItem>::new("docs adopt", Status::Issues, Vec::new()).errors(vec![reason]),
    )
}

async fn status(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
) -> Result<ExitCode> {
    let tools: Vec<&Tool> = manifest.tools.iter().collect();
    let probes = probe::probe_all(&tools, Platform::host()).await;
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

/// One search result. The recipes travel with it: a result set that only
/// says "now go and read the page" has moved the work, not done it.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchItem {
    pub name: String,
    pub score: u32,
    /// Which part of the entry matched: name, keyword, recipe, prose, tag, help.
    pub matched: String,
    pub what: String,
    /// `authored`, `draft`, or `none` when only the manifest matched.
    pub page: String,
    pub installed: bool,
    pub version: Option<String>,
    pub recipes: Vec<crate::core::docs::page::Recipe>,
}

async fn search(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
    args: &DocsSearchArgs,
) -> Result<ExitCode> {
    let tools = select(manifest, &args.tags, &None)?;
    let probes = probe::probe_all(&tools, Platform::host()).await;

    let entries: Vec<find::Entry<'_>> = tools
        .iter()
        .filter(|tool| {
            !args.installed
                || probes
                    .get(&tool.name)
                    .map(|probe| probe.installed)
                    .unwrap_or(false)
        })
        .map(|tool| find::Entry {
            tool,
            page: catalogue.get(&tool.name),
        })
        .collect();

    let query = args.query.join(" ");
    let hits = find::search(&entries, &query);

    let items: Vec<SearchItem> = hits
        .iter()
        .take(args.limit)
        .map(|hit| {
            let probe = probes.get(&hit.tool.name);
            SearchItem {
                name: hit.tool.name.clone(),
                score: hit.score,
                matched: hit.field.as_str().to_string(),
                what: match hit.page {
                    Some(page) => page.what.clone(),
                    None => hit.tool.summary.clone(),
                },
                page: match hit.page {
                    Some(page) => page.status.as_str().to_string(),
                    None => "none".to_string(),
                },
                installed: probe.map(|p| p.installed).unwrap_or(false),
                version: probe.and_then(|p| p.version.clone()),
                recipes: hit.recipes.iter().map(|r| (*r).clone()).collect(),
            }
        })
        .collect();

    if !ctx.json {
        if items.is_empty() {
            ctx.note(format!("nothing in the catalogue matches '{query}'"));
        }
        for item in &items {
            let marker = if item.installed { " " } else { "!" };
            println!("{marker} {:<20} {:<9} {}", item.name, item.page, item.what);
            for recipe in &item.recipes {
                println!("      $ {}", recipe.cmd);
                println!("        {}", recipe.does);
            }
        }
    }

    // Always Ok, even with no hits: an empty result is a correct answer, and
    // exit 3 here would teach every caller to write retry logic around a
    // working command.
    let shown = items.len();
    ctx.finish(
        Envelope::new("docs search", Status::Ok, items)
            .summary(serde_json::json!({ "query": query, "matched": hits.len(), "shown": shown })),
    )
}

fn dump(
    ctx: &Ctx,
    manifest: &crate::core::manifest::Manifest,
    catalogue: &Catalogue,
    args: &DocsDumpArgs,
) -> Result<ExitCode> {
    let tools = select(manifest, &args.tags, &args.profile)?;
    let items: Vec<ToolDoc> = tools
        .iter()
        .filter_map(|tool| catalogue.get(&tool.name))
        .cloned()
        .collect();

    if !ctx.json {
        println!("# The tools on this machine, and how to use them\n");
        for page in &items {
            println!(
                "## {}{}",
                page.name,
                if page.status == crate::core::docs::page::PageStatus::Draft {
                    " (draft)"
                } else {
                    ""
                }
            );
            println!("\n{}\n", page.what);
            if let Some(when) = &page.when {
                println!("{when}\n");
            }
            for recipe in &page.recipes {
                println!("- `{}` — {}", recipe.cmd, recipe.does);
            }
            if !page.recipes.is_empty() {
                println!();
            }
            if let Some(gotchas) = &page.gotchas {
                println!("{}\n", gotchas.trim());
            }
            if !page.see_also.is_empty() {
                println!("See also: {}\n", page.see_also.join(", "));
            }
        }
    }

    // Nothing is run here, ever: dump is the surface an agent pipes straight
    // into a context window, so it stays offline and deterministic.
    let total = items.len();
    ctx.finish(
        Envelope::new("docs dump", Status::Ok, items)
            .summary(serde_json::json!({ "pages": total, "tools": tools.len() })),
    )
}

/// Narrows the catalogue the same way every other command narrows the
/// manifest, so `--tag` means one thing across the CLI.
fn select<'m>(
    manifest: &'m crate::core::manifest::Manifest,
    tags: &[String],
    profile: &Option<String>,
) -> Result<Vec<&'m Tool>> {
    if tags.is_empty() && profile.is_none() {
        return Ok(manifest.tools.iter().collect());
    }
    Ok(graph::select(
        manifest,
        &Selection {
            profile: profile.clone(),
            tags: tags.to_vec(),
            names: Vec::new(),
        },
        Platform::host(),
    )?
    .tools)
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

/// What to do about a tool with no page — which depends on whether the
/// machine has anything to seed one from.
fn remediation(tool: &Tool) -> String {
    match capture::help_command(tool) {
        Some(_) => format!("run `pinst docs adopt {}` to seed a draft", tool.name),
        None => format!("write docs/tools/{}.toml by hand", tool.name),
    }
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

# Nothing on the machine, and nothing to ask: the only case that has to end in
# exit 3 rather than a capture.
[[tool]]
name = "oh-my-zsh"
summary = "A framework, not a command"
detect = { command = "false" }
help_cmd = ""
install = { method = "apt", packages = ["zsh"] }

[[tool]]
name = "seedable"
summary = "Answers when asked"
detect = { command = "true" }
help_cmd = "echo 'usage: seedable [OPTIONS]'"
install = { method = "apt", packages = ["seedable"] }
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
            platform: Platform::host(),
        }
    }

    fn show_args(tool: &str) -> DocsShowArgs {
        DocsShowArgs {
            tool: tool.to_string(),
            refresh: false,
        }
    }

    fn adopt_args(tool: &str) -> DocsAdoptArgs {
        DocsAdoptArgs {
            tool: tool.to_string(),
            refresh: false,
        }
    }

    #[tokio::test]
    async fn a_tool_with_a_page_exits_zero() {
        let cache = tempfile::tempdir().unwrap();
        let (_dir, catalogue) = catalogue(&[("ripgrep", r#"what = "Fast grep.""#)]);
        let code = show(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &show_args("ripgrep"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[tokio::test]
    async fn a_tool_with_neither_a_page_nor_any_help_is_something_to_act_on() {
        let cache = tempfile::tempdir().unwrap();
        let (_dir, catalogue) = catalogue(&[]);
        let code = show(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &show_args("oh-my-zsh"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Issues);
    }

    #[tokio::test]
    async fn a_tool_with_no_page_but_a_help_command_answers_from_the_capture() {
        let cache = tempfile::tempdir().unwrap();
        let (_dir, catalogue) = catalogue(&[]);

        // Exit 0: the question was answered. `source: captured` in the item is
        // what says it was answered by the tool rather than by a page.
        let code = show(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &show_args("seedable"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[tokio::test]
    async fn a_name_the_manifest_does_not_declare_is_a_usage_error() {
        let cache = tempfile::tempdir().unwrap();
        let (_dir, catalogue) = catalogue(&[]);
        let err = show(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &show_args("cowsay"),
        )
        .await
        .unwrap_err();
        assert!(
            err.downcast_ref::<UsageError>().is_some(),
            "an unknown tool must map to exit 2, not 1: {err:#}"
        );
    }

    #[test]
    fn the_remediation_depends_on_whether_anything_can_be_seeded_from() {
        // The exit-3 path is only useful if it says what to do about it, and
        // "run adopt" is wrong advice for a tool that answers nothing.
        let manifest = manifest();
        let seedable = lookup(&manifest, "seedable").unwrap();
        let silent = lookup(&manifest, "oh-my-zsh").unwrap();
        assert_eq!(
            remediation(seedable),
            "run `pinst docs adopt seedable` to seed a draft"
        );
        assert_eq!(
            remediation(silent),
            "write docs/tools/oh-my-zsh.toml by hand"
        );
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

    fn search_args(query: &str) -> DocsSearchArgs {
        DocsSearchArgs {
            query: query.split_whitespace().map(|s| s.to_string()).collect(),
            tags: Vec::new(),
            installed: false,
            limit: 10,
        }
    }

    #[tokio::test]
    async fn a_search_that_matches_nothing_still_succeeds() {
        // Exit 3 here would teach every caller to retry a command that worked.
        let (_dir, catalogue) = catalogue(&[]);
        let code = search(&ctx(), &manifest(), &catalogue, &search_args("kubernetes"))
            .await
            .unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[tokio::test]
    async fn a_search_finds_the_tool_and_succeeds() {
        let (_dir, catalogue) = catalogue(&[(
            "ripgrep",
            "what = \"Search a tree.\"\nkeywords = [\"grep\"]\n",
        )]);
        let code = search(&ctx(), &manifest(), &catalogue, &search_args("grep"))
            .await
            .unwrap();
        assert_eq!(code, ExitCode::Success);
    }

    #[tokio::test]
    async fn an_unknown_tag_is_a_usage_error_not_an_empty_result() {
        // Same rule as every other command that takes --tag: a filter nothing
        // can satisfy is a bad invocation.
        let (_dir, catalogue) = catalogue(&[]);
        let mut args = search_args("grep");
        args.tags = vec!["nonexistent".to_string()];
        let err = search(&ctx(), &manifest(), &catalogue, &args)
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<UsageError>().is_some(), "{err:#}");
    }

    #[test]
    fn dump_emits_every_page_of_the_selected_tools_exactly_once() {
        // `dump` is deliberately not async and takes no cache directory: it is
        // the surface an agent pipes into a context window, so it cannot run a
        // capture even by accident.
        let (_dir, catalogue) = catalogue(&[
            ("ripgrep", r#"what = "Search a tree.""#),
            ("fd", r#"what = "Find files.""#),
            ("seedable", r#"what = "Answers.""#),
        ]);
        let args = DocsDumpArgs {
            tags: Vec::new(),
            profile: None,
        };
        let code = dump(&ctx(), &manifest(), &catalogue, &args).unwrap();
        assert_eq!(code, ExitCode::Success);

        let manifest = manifest();
        let selected = select(&manifest, &args.tags, &args.profile).unwrap();
        let pages: Vec<&str> = selected
            .iter()
            .filter_map(|tool| catalogue.get(&tool.name))
            .map(|page| page.name.as_str())
            .collect();
        assert_eq!(pages.len(), 3);
        let mut unique = pages.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), pages.len(), "no page may appear twice");
    }

    #[test]
    fn a_tag_filter_narrows_what_dump_and_search_can_see() {
        let manifest = manifest();
        let all = select(&manifest, &[], &None).unwrap();
        let tagged = select(&manifest, &["dev".to_string()], &None).unwrap();
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].name, "ripgrep");
        assert!(tagged.len() < all.len());
    }

    #[tokio::test]
    async fn adopt_writes_a_draft_page_that_the_catalogue_can_load() {
        let cache = tempfile::tempdir().unwrap();
        let (dir, catalogue) = catalogue(&[]);

        let code = adopt(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("seedable"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Success);

        // The seed has to be a page the loader accepts — a draft nobody can
        // read back is worse than no draft.
        let reloaded = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        let page = reloaded.get("seedable").unwrap();
        assert_eq!(page.status, crate::core::docs::page::PageStatus::Draft);
        assert!(page.help.as_deref().unwrap().contains("usage: seedable"));
    }

    #[tokio::test]
    async fn adopt_under_dry_run_writes_nothing() {
        let cache = tempfile::tempdir().unwrap();
        let (dir, catalogue) = catalogue(&[]);

        let mut ctx = ctx();
        ctx.dry_run = true;
        let code = adopt(
            &ctx,
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("seedable"),
        )
        .await
        .unwrap();

        assert_eq!(code, ExitCode::Success);
        assert!(!dir.path().join("seedable.toml").exists());
    }

    #[tokio::test]
    async fn adopt_refuses_when_the_catalogue_came_out_of_the_binary() {
        // There is no tree to write into, and the refusal has to come before
        // anything is run.
        let cache = tempfile::tempdir().unwrap();
        let catalogue = Catalogue::load_from(Source::Embedded).unwrap();
        let code = adopt(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("seedable"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Issues);
    }

    #[tokio::test]
    async fn adopt_refuses_to_replace_authored_prose_with_captured_help() {
        let cache = tempfile::tempdir().unwrap();
        let (dir, catalogue) = catalogue(&[(
            "seedable",
            "what = \"Written by a person.\"\nstatus = \"authored\"\n",
        )]);
        let code = adopt(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("seedable"),
        )
        .await
        .unwrap();

        assert_eq!(code, ExitCode::Issues);
        let after = std::fs::read_to_string(dir.path().join("seedable.toml")).unwrap();
        assert!(after.contains("Written by a person."), "{after}");
    }

    #[tokio::test]
    async fn adopt_needs_yes_to_re_seed_an_existing_draft() {
        let cache = tempfile::tempdir().unwrap();
        let (dir, catalogue) = catalogue(&[("seedable", r#"what = "An earlier draft.""#)]);

        let refused = adopt(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("seedable"),
        )
        .await
        .unwrap();
        assert_eq!(refused, ExitCode::Issues);
        assert!(
            std::fs::read_to_string(dir.path().join("seedable.toml"))
                .unwrap()
                .contains("An earlier draft.")
        );

        let mut ctx = ctx();
        ctx.yes = true;
        let code = adopt(
            &ctx,
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("seedable"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Success);
        assert!(
            std::fs::read_to_string(dir.path().join("seedable.toml"))
                .unwrap()
                .contains("usage: seedable")
        );
    }

    #[tokio::test]
    async fn adopt_refuses_when_there_is_nothing_to_capture() {
        let cache = tempfile::tempdir().unwrap();
        let (dir, catalogue) = catalogue(&[]);

        let code = adopt(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("oh-my-zsh"),
        )
        .await
        .unwrap();
        assert_eq!(code, ExitCode::Issues);
        assert!(!dir.path().join("oh-my-zsh.toml").exists());
    }

    #[tokio::test]
    async fn adopt_of_an_unknown_tool_is_a_usage_error() {
        let cache = tempfile::tempdir().unwrap();
        let (_dir, catalogue) = catalogue(&[]);
        let err = adopt(
            &ctx(),
            &manifest(),
            &catalogue,
            cache.path(),
            &adopt_args("cowsay"),
        )
        .await
        .unwrap_err();
        assert!(err.downcast_ref::<UsageError>().is_some(), "{err:#}");
    }

    #[test]
    fn coverage_classifies_every_tool_and_counts_the_gaps() {
        let (_dir, catalogue) = catalogue(&[
            ("ripgrep", "what = \"Fast grep.\"\nstatus = \"authored\"\n"),
            ("fd", r#"what = "Fast find.""#),
        ]);
        let (items, orphans) = coverage(&manifest(), &catalogue, &Default::default());

        assert_eq!(items.len(), manifest().tools.len());
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
