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
            // `;` not `&&`: the staged download is cleaned up even when the
            // extraction fails, so a retry does not pile up /tmp files.
            Action::Shell {
                command: format!(
                    "{sudo}tar -C {dest} -xzf {staged}; rm -f {staged}",
                    staged = staged.display()
                ),
            },
        ])
    }
}

/// Destinations inside `$HOME` are ours to write; anything else (`/opt`,
/// `/usr/local`) needs elevation.
fn under_home(dest: &str) -> bool {
    // A manifest may write the destination the way the shell that runs the
    // step will read it — `$HOME/.local/bin`, `~/.local/bin` — as the
    // git_clone entries already do. Those are under `$HOME` by construction,
    // and missing that would elevate a write into the user's own home and
    // leave the result root-owned.
    for prefix in ["$HOME/", "${HOME}/", "~/"] {
        if dest.starts_with(prefix) {
            return true;
        }
    }
    if matches!(dest, "$HOME" | "${HOME}" | "~") {
        return true;
    }
    std::env::var_os("HOME")
        .map(|home| std::path::Path::new(dest).starts_with(std::path::Path::new(&home)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::under_home;

    #[test]
    fn recognizes_unexpanded_home_destinations() {
        // Elevating for these would write into the user's own home as root.
        assert!(under_home("$HOME/.local/bin"));
        assert!(under_home("${HOME}/.local/bin"));
        assert!(under_home("~/.local/bin"));
        // And the cases that genuinely need sudo stay untouched.
        assert!(!under_home("/opt"));
        assert!(!under_home("/usr/local/bin"));
    }
}
