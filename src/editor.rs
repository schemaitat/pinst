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
pub fn launch(terminal: &mut DefaultTerminal, path: &Path) -> bool {
    let editor_cmd = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
    let mut parts = editor_cmd.split_whitespace();
    let program = parts.next().unwrap_or("nvim").to_string();
    let args: Vec<String> = parts.map(str::to_string).collect();

    ratatui::restore();
    let status = Command::new(&program).args(&args).arg(path).status();
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

    matches!(status, Ok(s) if s.success())
}
