//! The command surface. Thin: every command parses arguments, calls into
//! `core`, and renders — no logic lives here.

pub mod commands;
pub mod output;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use color_eyre::eyre::Result;

use output::{Ctx, ExitCode};

#[derive(Debug, Parser)]
#[command(
    name = "pinst",
    version,
    about = "Self-contained toolchain and dotfiles manager",
    long_about = "pinst installs, updates, and doctors this machine's toolchain from a \
                  manifest, and lays down the configs it carries embedded. Every command \
                  supports --json and --dry-run; `pinst schema output` is the machine contract.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Args, Clone)]
pub struct GlobalArgs {
    /// Emit a machine-readable JSON envelope on stdout (implies non-interactive).
    #[arg(long, global = true)]
    pub json: bool,
    /// Show what would happen without changing anything.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Authorize privileged and confirmation-gated steps without prompting.
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,
    /// Suppress human progress output on stderr.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,
    /// Use this manifest instead of the discovered/embedded one.
    #[arg(long, global = true, value_name = "PATH")]
    pub manifest: Option<PathBuf>,
}

impl GlobalArgs {
    pub fn ctx(&self) -> Ctx {
        Ctx {
            json: self.json,
            dry_run: self.dry_run,
            yes: self.yes,
            quiet: self.quiet,
            manifest_path: self.manifest.clone(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// List the manifest's tools with their live installation status.
    List(SelectArgs),
    /// Show the ordered plan for installing the selected tools.
    Plan(SelectArgs),
    /// Install missing tools.
    Install(SelectArgs),
    /// Upgrade installed tools that have a newer version available.
    Update(SelectArgs),
    /// Manage the configs pinst carries (status, apply, diff, adopt).
    Config(ConfigArgs),
    /// Look up how to use the tools the manifest declares.
    Docs(DocsArgs),
    /// Check tools and configs, reporting actionable findings.
    Doctor(DoctorArgs),
    /// Validate and measure the agent harness: the .ash/ plan corpus.
    Harness(HarnessArgs),
    /// Converge this machine: install tools, apply configs, then report.
    Apply(SelectArgs),
    /// One-shot provisioning for a fresh machine.
    Bootstrap(SelectArgs),
    /// Emit machine-readable schemas.
    Schema(SchemaArgs),
    /// Launch the interactive dashboard.
    Tui,
}

impl Commands {
    pub fn name(&self) -> &'static str {
        match self {
            Commands::List(_) => "list",
            Commands::Plan(_) => "plan",
            Commands::Install(_) => "install",
            Commands::Update(_) => "update",
            Commands::Config(_) => "config",
            Commands::Docs(_) => "docs",
            Commands::Doctor(_) => "doctor",
            Commands::Harness(_) => "harness",
            Commands::Apply(_) => "apply",
            Commands::Bootstrap(_) => "bootstrap",
            Commands::Schema(_) => "schema",
            Commands::Tui => "tui",
        }
    }
}

/// Narrows the tool set. With nothing given, the whole manifest is selected;
/// named tools always pull in their dependencies.
#[derive(Debug, Args, Clone, Default)]
pub struct SelectArgs {
    /// Tool names. Defaults to every tool in the manifest.
    pub tools: Vec<String>,
    /// Select every tool carrying the tags of this profile.
    #[arg(long)]
    pub profile: Option<String>,
    /// Select every tool carrying this tag (repeatable).
    #[arg(long = "tag")]
    pub tags: Vec<String>,
}

impl SelectArgs {
    pub fn selection(&self) -> crate::core::graph::Selection {
        crate::core::graph::Selection {
            profile: self.profile.clone(),
            tags: self.tags.clone(),
            names: self.tools.clone(),
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ConfigAction {
    /// Report each config file's state in $HOME.
    Status,
    /// Link or materialize the configs into $HOME.
    Apply,
    /// Show which files differ from what pinst carries.
    Diff,
    /// Copy on-disk edits back into the source tree.
    Adopt,
}

/// The tool catalogue: what each tool is for, and what to type.
///
/// Shaped for a caller that has a task rather than a tool in mind — which is
/// why `search` returns the matching recipes inline instead of a list of names
/// to look up separately.
#[derive(Debug, Args, Clone)]
pub struct DocsArgs {
    #[command(subcommand)]
    pub action: DocsAction,
}

#[derive(Debug, Subcommand, Clone)]
pub enum DocsAction {
    /// Find the tool for a task, with the matching recipes inline.
    Search(DocsSearchArgs),
    /// Show the page for one tool.
    Show(DocsShowArgs),
    /// Print the whole catalogue as one document.
    Dump(DocsDumpArgs),
    /// Report which tools have a page, and which are still drafts.
    Status,
    /// Seed a draft page from a tool's own help output.
    Adopt(DocsAdoptArgs),
}

#[derive(Debug, Args, Clone)]
pub struct DocsSearchArgs {
    /// What you are trying to do, in your own words.
    #[arg(required = true, num_args = 1..)]
    pub query: Vec<String>,
    /// Only tools carrying this tag (repeatable).
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Only tools that are actually installed.
    #[arg(long)]
    pub installed: bool,
    /// Keep at most this many results.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

#[derive(Debug, Args, Clone)]
pub struct DocsDumpArgs {
    /// Only tools carrying this tag (repeatable).
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Only tools selected by this profile.
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct DocsShowArgs {
    /// The tool to look up. Must be a tool the manifest declares.
    pub tool: String,
    /// Re-run the tool's help instead of answering from the cache.
    #[arg(long)]
    pub refresh: bool,
}

#[derive(Debug, Args, Clone)]
pub struct DocsAdoptArgs {
    /// The tool to seed a page for.
    pub tool: String,
    /// Re-run the tool's help instead of seeding from the cache.
    #[arg(long)]
    pub refresh: bool,
}

/// The agent harness: the checks over `.ash/` and `.agents/skills/`.
///
/// Unlike every other family here, this one reads the repo the caller is
/// standing in rather than the machine — so it loads no manifest, and its
/// corpus is discovered by walking up for `.ash/` rather than by looking
/// beside a `manifest.toml`.
#[derive(Debug, Args, Clone)]
pub struct HarnessArgs {
    /// The repo whose corpus to read. Defaults to $PINST_ASH_DIR, else the
    /// nearest ancestor of the working directory holding a `.ash/`.
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,
    #[command(subcommand)]
    pub action: HarnessAction,
}

#[derive(Debug, Subcommand, Clone)]
pub enum HarnessAction {
    /// Validate every corpus invariant.
    Check,
    /// Regenerate .ash/INDEX.md from the plan frontmatter.
    Index(HarnessIndexArgs),
    /// Grade each skill against the contract it declares.
    Skills(HarnessSkillsArgs),
    /// Mint a plan id: <yymmdd>-<six letters>.
    NewId(HarnessNewIdArgs),
    /// Move a lesson to a free LESSON-NNN, taking this branch's citations.
    RenumberLesson(HarnessRenumberLessonArgs),
}

#[derive(Debug, Args, Clone)]
pub struct HarnessIndexArgs {
    /// Report whether the index is up to date instead of rewriting it.
    #[arg(long)]
    pub check: bool,
}

/// The graded report. Deliberately not part of `just qc`: these are rates,
/// and a rate that got interesting would fail the build.
#[derive(Debug, Args, Clone)]
pub struct HarnessSkillsArgs {
    /// Skip the `evidence:` commands instead of running them. They are shell
    /// taken from the repo being checked; see `pinst harness skills --help`.
    #[arg(long)]
    pub no_evidence: bool,
    /// Also count invocations from a runtime's session transcripts. Opt-in,
    /// and never depended on: it corroborates, it does not decide.
    #[arg(long, value_name = "DIR")]
    pub transcripts: Option<PathBuf>,
}

/// The remedy for `lesson.duplicate-id`, which until now was "grep the corpus
/// by hand" (LESSON-022).
///
/// The lesson is named by a substring of its *title*, not by its id: both
/// headings in a collision carry the same id, so an id could not say which of
/// them to move.
#[derive(Debug, Args, Clone)]
pub struct HarnessRenumberLessonArgs {
    /// A substring of the heading's title, case-insensitive. Must match
    /// exactly one heading.
    #[arg(long)]
    pub title: String,
    /// Move it to this id instead of the next free one.
    #[arg(long, value_name = "LESSON-NNN")]
    pub to: Option<String>,
    /// Take the merge-base against this ref when deciding which citations
    /// this branch introduced. Defaults to `main`.
    #[arg(long, value_name = "REF")]
    pub against: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct HarnessNewIdArgs {
    /// Use this date (YYYY-MM-DD) instead of today.
    pub date: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct DoctorArgs {
    /// Apply the fixable subset of findings.
    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Args, Clone)]
pub struct SchemaArgs {
    #[arg(value_enum, default_value_t = SchemaKind::Manifest)]
    pub kind: SchemaKind,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum SchemaKind {
    /// JSON Schema for manifest.toml.
    Manifest,
    /// JSON Schema for the --json output envelope.
    Output,
    /// JSON Schema for a docs/tools/<name>.toml page.
    Docs,
    /// JSON Schema for a `pinst harness skills` report row.
    Harness,
}

pub async fn dispatch(cli: Cli) -> Result<ExitCode> {
    let ctx = cli.global.ctx();
    match &cli.command {
        Commands::List(args) => commands::list::run(&ctx, args).await,
        Commands::Plan(args) => commands::plan::run(&ctx, args).await,
        Commands::Install(args) => commands::install::run(&ctx, args).await,
        Commands::Update(args) => commands::update::run(&ctx, args).await,
        Commands::Config(args) => commands::config::run(&ctx, args).await,
        Commands::Docs(args) => commands::docs::run(&ctx, args).await,
        Commands::Doctor(args) => commands::doctor::run(&ctx, args).await,
        Commands::Harness(args) => commands::harness::run(&ctx, args),
        Commands::Apply(args) => commands::apply::run(&ctx, args).await,
        Commands::Bootstrap(args) => commands::bootstrap::run(&ctx, args).await,
        Commands::Schema(args) => commands::schema::run(&ctx, args),
        Commands::Tui => commands::tui::run(&ctx).await,
    }
}
