use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

/// Stage and verify a release before extracting it, elevating only when
/// the destination demands it. Cleanup never masks the operation's status.
pub struct GithubRelease;

impl Executor for GithubRelease {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::GithubRelease {
            repo,
            asset,
            dest,
            version,
            sha256,
            checksum_asset,
            ..
        } = install
        else {
            bail!("github_release executor called for tool '{}'", tool.name);
        };

        let release = version
            .as_ref()
            .map(|tag| format!("download/{tag}"))
            .unwrap_or_else(|| "latest/download".into());
        let base = format!("https://github.com/{repo}/releases/{release}");
        let url = format!("{base}/{asset}");
        let sudo = if under_home(dest) { "" } else { "sudo " };
        let verify = if let Some(hash) = sha256 {
            super::verify_hash(hash, "\"$tmp/archive\"")?
        } else if let Some(checksum) = checksum_asset {
            format!(
                "{} {} -o \"$tmp/checksum\"; expected=$(awk 'NR==1 {{print $1}}' \"$tmp/checksum\"); if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum \"$tmp/archive\"); else actual=$(shasum -a 256 \"$tmp/archive\"); fi; [ \"${{actual%% *}}\" = \"$expected\" ] || {{ echo 'SHA-256 mismatch' >&2; exit 1; }}",
                super::CURL,
                super::quote(&format!("{base}/{checksum}"))
            )
        } else {
            ":".into()
        };
        Ok(vec![Action::Shell {
            command: format!(
            "set -eu; tmp=$(mktemp -d); trap 'rm -rf \"$tmp\"' EXIT; {} {} -o \"$tmp/archive\"; {verify}; mkdir \"$tmp/unpacked\"; tar -C \"$tmp/unpacked\" -xzf \"$tmp/archive\"; dest={dest}; {sudo}mkdir -p \"$dest\"; {sudo}cp -Rf \"$tmp/unpacked/.\" \"$dest/\"",
                super::CURL,
                super::quote(&url)
            ),
        }])
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
