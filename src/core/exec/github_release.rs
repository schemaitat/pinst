use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

/// Installs from a GitHub release asset: download natively (pinst already
/// speaks HTTP for upgrade checks, so this needs no `curl` on the box), then
/// extract with `tar`, elevating only when the destination demands it.
pub struct GithubRelease;

impl Executor for GithubRelease {
    fn install(&self, tool: &Tool) -> Result<Vec<Action>> {
        let Install::GithubRelease {
            repo, asset, dest, ..
        } = &tool.install
        else {
            bail!("github_release executor called for tool '{}'", tool.name);
        };

        let url = format!("https://github.com/{repo}/releases/latest/download/{asset}");
        let staged = std::env::temp_dir().join(format!("pinst-{}-{asset}", tool.name));
        let sudo = if under_home(dest) { "" } else { "sudo " };

        Ok(vec![
            Action::Download {
                url,
                dest: staged.clone(),
            },
            Action::Shell {
                command: format!("{sudo}mkdir -p {dest}"),
            },
            Action::Shell {
                command: format!("{sudo}tar -C {dest} -xzf {}", staged.display()),
            },
            Action::Remove { path: staged },
        ])
    }
}

/// Destinations inside `$HOME` are ours to write; anything else (`/opt`,
/// `/usr/local`) needs elevation.
fn under_home(dest: &str) -> bool {
    std::env::var_os("HOME")
        .map(|home| std::path::Path::new(dest).starts_with(std::path::Path::new(&home)))
        .unwrap_or(false)
}
