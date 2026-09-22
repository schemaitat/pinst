//! Config quick-access (TASK-014): suspends the TUI, hands the terminal to
//! `$EDITOR`, and cleanly reclaims it on return.

use std::io::stdout;
use std::path::Path;
use std::process::Command;

use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use ratatui::DefaultTerminal;

/// Leaves the alternate screen / raw mode, blocks on the editor subprocess,
/// then re-initializes the terminal and forces a full redraw. Errors from
/// the editor itself are surfaced via the caller's status line rather than
/// aborting pinst.
pub fn launch(terminal: &mut DefaultTerminal, path: &Path) -> Result<(), String> {
    let editor_cmd = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
    ratatui::restore();
    let status = run_command(&editor_cmd, path);
    *terminal = ratatui::init();
    // Re-entering the alternate screen can resurface whatever was on it
    // before we left (some terminals preserve the alt-screen buffer across
    // enter/leave), and cells that are blank in the next frame wouldn't
    // otherwise get touched by ratatui's diffed redraw. `Terminal::clear()`
    // would handle this, but it round-trips a cursor-position query first
    // and silently no-ops the whole clear if that query fails/goes
    // unanswered (some terminals/multiplexers won't answer it) — so clear
    // the physical screen directly instead, which has no such dependency.
    let _ = execute!(stdout(), Clear(ClearType::All));

    status
}

fn run_command(editor_cmd: &str, path: &Path) -> Result<(), String> {
    let mut parts = editor_cmd.split_whitespace();
    let program = parts.next().unwrap_or("nvim");
    let args: Vec<&str> = parts.collect();
    match Command::new(program).args(args).arg(path).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{program} exited with {status}")),
        Err(error) => Err(format!("could not launch {program}: {error}")),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::run_command;

    fn script(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("editor");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        (dir, path)
    }

    #[test]
    fn editor_exit_status_is_reported() {
        let (_dir, success) = script("exit 0");
        assert!(run_command(success.to_str().unwrap(), Path::new("page.toml")).is_ok());

        let (_dir, failure) = script("exit 7");
        let error = run_command(failure.to_str().unwrap(), Path::new("page.toml")).unwrap_err();
        assert!(error.contains("exit status: 7"), "{error}");
    }
}
