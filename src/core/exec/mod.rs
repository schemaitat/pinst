//! Install-method executors and the one runner every action funnels through.
//!
//! Adding a new install method means adding a variant to `manifest::Install`,
//! a module here implementing [`Executor`], and a line in [`executor_for`] —
//! nothing else in the engine needs to change.

mod apt;
mod cargo;
mod curl_script;
mod git_clone;
mod github_release;
mod nvm;
mod shell;

use std::io::Write;
use std::process::{Command, Stdio};

use color_eyre::eyre::{Context, Result, bail};

use super::manifest::{Install, Tool};
use super::plan::Action;

/// Produces the actions that install or upgrade one tool. One implementation
/// per install method, each ignorant of every other method.
pub trait Executor {
    fn install(&self, tool: &Tool) -> Result<Vec<Action>>;

    /// Most methods upgrade by re-running their install (apt/cargo/curl
    /// installers are all idempotent-ish in that way); methods that need
    /// something different override this.
    fn upgrade(&self, tool: &Tool) -> Result<Vec<Action>> {
        self.install(tool)
    }

    /// Set when the method cannot be automated at all.
    fn blocked_reason(&self, _tool: &Tool) -> Option<String> {
        None
    }
}

pub fn executor_for(install: &Install) -> Box<dyn Executor> {
    match install {
        Install::Apt { .. } => Box::new(apt::Apt),
        Install::Cargo { .. } => Box::new(cargo::Cargo),
        Install::CurlScript { .. } => Box::new(curl_script::CurlScript),
        Install::Shell { .. } => Box::new(shell::ShellInstall),
        Install::GithubRelease { .. } => Box::new(github_release::GithubRelease),
        Install::Nvm { .. } => Box::new(nvm::Nvm),
        Install::GitClone { .. } => Box::new(git_clone::GitClone),
        Install::Manual { .. } => Box::new(Manual),
    }
}

struct Manual;

impl Executor for Manual {
    fn install(&self, _tool: &Tool) -> Result<Vec<Action>> {
        Ok(Vec::new())
    }

    fn blocked_reason(&self, tool: &Tool) -> Option<String> {
        match &tool.install {
            Install::Manual { note } => Some(note.clone()),
            _ => None,
        }
    }
}

/// Executes actions. The single place where pinst touches the system, so
/// dry-run and privilege handling cannot be forgotten by an individual
/// executor.
pub struct Runner {
    pub dry_run: bool,
}

impl Runner {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }

    pub fn run(&self, action: &Action) -> Result<()> {
        if self.dry_run {
            return Ok(());
        }
        match action {
            Action::Shell { command } => run_shell(command),
            Action::Download { url, dest } => download(url, dest),
            Action::Link { source, target } => link(source, target),
            Action::Write {
                target, content, ..
            } => write_file(target, content),
            Action::Backup { path, to } => {
                std::fs::rename(path, to).with_context(|| {
                    format!("backing up {} to {}", path.display(), to.display())
                })?;
                Ok(())
            }
            Action::Remove { path } => {
                // Already gone is success, not an error: uninstall run twice
                // is a normal thing to do, and failing on a path the first
                // run already removed would push people toward `rm -rf`
                // instead of trusting the command a second time.
                if std::fs::symlink_metadata(path).is_err() {
                    return Ok(());
                }
                remove_any(path)
            }
        }
    }
}

/// Runs a `sh -c` check and reports whether it succeeded. Used for
/// `detect.command` and post-install `skip_if` conditions.
pub fn check(command: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_shell(command: &str) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("spawning: {command}"))?;
    if !status.success() {
        bail!(
            "command failed ({}): {command}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string())
        );
    }
    Ok(())
}

fn download(url: &str, dest: &std::path::Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("pinst")
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("downloading {url}"))?;
    if !resp.status().is_success() {
        bail!("downloading {url}: HTTP {}", resp.status());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file =
        std::fs::File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    resp.copy_to(&mut file)?;
    file.flush()?;
    Ok(())
}

fn link(source: &std::path::Path, target: &std::path::Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Replace an existing link; callers back up plain files before this runs.
    if std::fs::symlink_metadata(target).is_ok() {
        remove_any(target)?;
    }
    std::os::unix::fs::symlink(source, target)
        .with_context(|| format!("linking {} -> {}", target.display(), source.display()))?;
    Ok(())
}

fn write_file(target: &std::path::Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Replace rather than truncate in place: the target may currently be a
    // symlink into a source tree we must not write through.
    if std::fs::symlink_metadata(target).is_ok() {
        remove_any(target)?;
    }
    std::fs::write(target, content).with_context(|| format!("writing {}", target.display()))?;
    Ok(())
}

fn remove_any(path: &std::path::Path) -> Result<()> {
    let meta =
        std::fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .with_context(|| format!("removing {}", path.display()))?;
    Ok(())
}
