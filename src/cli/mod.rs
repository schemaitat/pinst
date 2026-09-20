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
    /// Check tools and configs, reporting actionable findings.
    Doctor(DoctorArgs),
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
            Commands::Doctor(_) => "doctor",
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
}

pub async fn dispatch(cli: Cli) -> Result<ExitCode> {
    let ctx = cli.global.ctx();
    match &cli.command {
        Commands::List(args) => commands::list::run(&ctx, args).await,
        Commands::Plan(args) => commands::plan::run(&ctx, args).await,
        Commands::Install(args) => commands::install::run(&ctx, args).await,
        Commands::Update(args) => commands::update::run(&ctx, args).await,
        Commands::Config(args) => commands::config::run(&ctx, args).await,
        Commands::Doctor(args) => commands::doctor::run(&ctx, args).await,
        Commands::Apply(args) => commands::apply::run(&ctx, args).await,
        Commands::Bootstrap(args) => commands::bootstrap::run(&ctx, args).await,
        Commands::Schema(args) => commands::schema::run(&ctx, args),
        Commands::Tui => commands::tui::run(&ctx).await,
    }
}
