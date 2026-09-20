//! What the machine can say about a tool that nobody has written a page for.
//!
//! This is the degraded answer, not the good one: `--help` is written for
//! someone who already knows they want this tool. It exists so that an
//! undocumented tool still returns something, and so that `docs adopt` can
//! seed a page from the tool's real output rather than from memory.
//!
//! Three constraints shape everything here. It runs an installed binary, so
//! it only ever runs the command the *manifest* declares for a tool the
//! manifest declares — never a name that came from a query. It runs with a
//! timeout and a byte cap, because `--help` is not a contract and a tool that
//! decides to page, prompt or stream would otherwise hang the caller. And it
//! caches against the probed version, which is the one key that goes stale
//! exactly when the answer does.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use color_eyre::eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::core::manifest::Tool;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// Enough for any reasonable `--help`, small enough that a tool streaming its
/// entire manual cannot fill a context window or a cache directory.
const MAX_BYTES: usize = 64 * 1024;

/// One tool's captured help, as cached.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capture {
    /// The command that produced this, so a reader can reproduce it.
    pub command: String,
    /// The version the capture was taken against. A different one is a miss:
    /// help text is a property of the installed binary.
    pub version: Option<String>,
    pub text: String,
    /// True when the output hit `MAX_BYTES` and was cut.
    pub truncated: bool,
}

/// How to ask a tool to describe itself: the manifest's override, else
/// `<bin> --help`. `None` means there is nothing to ask — an oh-my-zsh plugin
/// or a metapackage has no command of its own.
pub fn help_command(tool: &Tool) -> Option<String> {
    if let Some(explicit) = &tool.help_cmd {
        let trimmed = explicit.trim();
        // An empty override is how the manifest says "this one has no help",
        // distinct from saying nothing and falling back to the binary.
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    tool.detect.bin.as_ref().map(|bin| format!("{bin} --help"))
}

/// The capture for a tool, from the cache when it is still valid, otherwise by
/// running the tool. `None` when the tool has no help command or it produced
/// nothing.
pub async fn capture(
    tool: &Tool,
    version: Option<&str>,
    refresh: bool,
    cache_dir: &Path,
) -> Result<Option<Capture>> {
    capture_with(
        tool,
        version,
        refresh,
        cache_dir,
        CAPTURE_TIMEOUT,
        MAX_BYTES,
    )
    .await
}

/// The body of `capture`, with the limits as parameters so the tests can
/// exercise the timeout and the cap without waiting ten seconds or generating
/// 64 KiB.
pub(crate) async fn capture_with(
    tool: &Tool,
    version: Option<&str>,
    refresh: bool,
    cache_dir: &Path,
    limit: Duration,
    max_bytes: usize,
) -> Result<Option<Capture>> {
    let Some(command) = help_command(tool) else {
        return Ok(None);
    };

    let path = cache_path(cache_dir, &tool.name);
    if !refresh && let Some(cached) = read_cache(&path) {
        // The command is part of the key: a manifest that now asks a different
        // question must not be answered from the old one.
        if cached.version.as_deref() == version && cached.command == command {
            return Ok(Some(cached));
        }
    }

    let Some((text, truncated)) = run(&command, limit, max_bytes).await else {
        return Ok(None);
    };

    let capture = Capture {
        command,
        version: version.map(|v| v.to_string()),
        text,
        truncated,
    };
    write_cache(&path, &capture)?;
    Ok(Some(capture))
}

/// `${XDG_CACHE_HOME:-$HOME/.cache}/pinst/docs`.
pub fn cache_dir() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_CACHE_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => crate::core::home_dir()?.join(".cache"),
    };
    Ok(base.join("pinst").join("docs"))
}

fn cache_path(cache_dir: &Path, tool: &str) -> PathBuf {
    cache_dir.join(format!("{tool}.json"))
}

/// A cache miss and a corrupt cache are the same thing to a caller: run it
/// again. Nothing here is worth failing a lookup over.
fn read_cache(path: &Path) -> Option<Capture> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(path: &Path, capture: &Capture) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(capture)?;
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Runs the help command and returns its output, or `None` when it said
/// nothing worth keeping.
///
/// Exit status is deliberately ignored: plenty of tools print their usage and
/// exit non-zero, and a usage block is exactly what is wanted here.
async fn run(command: &str, limit: Duration, max_bytes: usize) -> Option<(String, bool)> {
    let result = timeout(
        limit,
        Command::new("sh")
            .arg("-c")
            .arg(command)
            // A tool that decides it is talking to a terminal would otherwise
            // put escape sequences into a page and into an agent's context.
            .env("NO_COLOR", "1")
            .env("TERM", "dumb")
            .stdin(Stdio::null())
            .output(),
    )
    .await;

    let output = match result {
        Ok(Ok(output)) => output,
        // A timeout or a failure to spawn both mean "no help available"; the
        // caller reports the gap rather than the mechanism.
        _ => return None,
    };

    // Usage goes to stderr about as often as to stdout.
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    if text.trim().is_empty() {
        return None;
    }

    Some(truncate(text, max_bytes))
}

/// Cuts at a character boundary, and says so in the text: a page that was
/// silently halved reads as a complete one.
fn truncate(text: String, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = text[..end].to_string();
    cut.push_str("\n[... truncated by pinst ...]\n");
    (cut, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest::{Detect, Install};

    fn tool(name: &str, bin: Option<&str>, help_cmd: Option<&str>) -> Tool {
        Tool {
            name: name.to_string(),
            summary: String::new(),
            tags: Vec::new(),
            requires: Vec::new(),
            detect: Detect {
                command: "true".to_string(),
                bin: bin.map(|b| b.to_string()),
                version_cmd: None,
                version_regex: None,
            },
            help_cmd: help_cmd.map(|c| c.to_string()),
            install: Install::Manual {
                note: String::new(),
            },
            upgrade: None,
            post_install: Vec::new(),
        }
    }

    #[test]
    fn the_help_command_defaults_to_the_detected_binary() {
        assert_eq!(
            help_command(&tool("fd", Some("fd"), None)).as_deref(),
            Some("fd --help")
        );
    }

    #[test]
    fn the_manifest_can_override_the_help_command() {
        assert_eq!(
            help_command(&tool("nvm", Some("nvm"), Some("nvm --help"))).as_deref(),
            Some("nvm --help")
        );
    }

    #[test]
    fn a_tool_with_no_binary_has_no_help_to_capture() {
        assert!(help_command(&tool("oh-my-zsh", None, None)).is_none());
    }

    #[test]
    fn an_empty_override_means_this_tool_has_no_help() {
        // Distinct from saying nothing, which falls back to the binary.
        assert!(help_command(&tool("zsh-autosuggestions", Some("zsh"), Some("  "))).is_none());
    }

    #[tokio::test]
    async fn a_capture_is_written_to_the_cache_and_read_back_without_rerunning() {
        let dir = tempfile::tempdir().unwrap();
        // Output that changes every run, so a second identical answer can only
        // have come from the cache.
        let tool = tool("noisy", None, Some("date +%s%N"));

        let first = capture(&tool, Some("1.0"), false, dir.path())
            .await
            .unwrap()
            .unwrap();
        let second = capture(&tool, Some("1.0"), false, dir.path())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first, second, "the second call must not re-run the tool");
        assert!(cache_path(dir.path(), "noisy").is_file());
    }

    #[tokio::test]
    async fn a_new_version_invalidates_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let tool = tool("noisy", None, Some("date +%s%N"));

        let first = capture(&tool, Some("1.0"), false, dir.path())
            .await
            .unwrap()
            .unwrap();
        let after_upgrade = capture(&tool, Some("2.0"), false, dir.path())
            .await
            .unwrap()
            .unwrap();

        assert_ne!(first.text, after_upgrade.text);
        assert_eq!(after_upgrade.version.as_deref(), Some("2.0"));
    }

    #[tokio::test]
    async fn refresh_reruns_a_valid_cache_entry() {
        let dir = tempfile::tempdir().unwrap();
        let tool = tool("noisy", None, Some("date +%s%N"));

        let first = capture(&tool, Some("1.0"), false, dir.path())
            .await
            .unwrap()
            .unwrap();
        let forced = capture(&tool, Some("1.0"), true, dir.path())
            .await
            .unwrap()
            .unwrap();

        assert_ne!(first.text, forced.text);
    }

    #[tokio::test]
    async fn a_changed_help_command_invalidates_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let before = capture(
            &tool("t", None, Some("echo old")),
            Some("1.0"),
            false,
            dir.path(),
        )
        .await
        .unwrap()
        .unwrap();
        let after = capture(
            &tool("t", None, Some("echo new")),
            Some("1.0"),
            false,
            dir.path(),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(before.text.trim(), "old");
        assert_eq!(after.text.trim(), "new");
    }

    #[tokio::test]
    async fn a_command_that_hangs_is_reported_as_no_help_rather_than_hanging() {
        let dir = tempfile::tempdir().unwrap();
        let result = capture_with(
            &tool("slow", None, Some("sleep 30")),
            None,
            false,
            dir.path(),
            Duration::from_millis(100),
            MAX_BYTES,
        )
        .await
        .unwrap();
        assert!(result.is_none());
        assert!(
            !cache_path(dir.path(), "slow").exists(),
            "nothing useful was captured, so nothing should be cached"
        );
    }

    #[tokio::test]
    async fn output_past_the_cap_is_truncated_rather_than_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let captured = capture_with(
            &tool("loud", None, Some("yes usage | head -c 4096")),
            None,
            false,
            dir.path(),
            CAPTURE_TIMEOUT,
            128,
        )
        .await
        .unwrap()
        .unwrap();

        assert!(captured.truncated);
        assert!(captured.text.contains("truncated by pinst"));
        assert!(captured.text.len() < 256);
    }

    #[tokio::test]
    async fn usage_printed_to_stderr_is_still_a_capture() {
        let dir = tempfile::tempdir().unwrap();
        let captured = capture(
            &tool(
                "grumpy",
                None,
                Some("echo 'usage: grumpy [opts]' >&2; exit 1"),
            ),
            None,
            false,
            dir.path(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(captured.text.contains("usage: grumpy"));
    }

    #[tokio::test]
    async fn a_command_that_says_nothing_captures_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let captured = capture(&tool("mute", None, Some("true")), None, false, dir.path())
            .await
            .unwrap();
        assert!(captured.is_none());
    }

    #[tokio::test]
    async fn a_tool_with_no_help_command_captures_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let captured = capture(&tool("oh-my-zsh", None, None), None, false, dir.path())
            .await
            .unwrap();
        assert!(captured.is_none());
    }

    #[tokio::test]
    async fn a_corrupt_cache_entry_is_a_miss_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(cache_path(dir.path(), "t"), "{ not json").unwrap();

        let captured = capture(&tool("t", None, Some("echo fine")), None, false, dir.path())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured.text.trim(), "fine");
    }
}
