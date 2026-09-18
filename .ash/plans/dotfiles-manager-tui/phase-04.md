# Phase 4 — Config quick-access / editor launch

## Status
Done

## Goal
GOAL-004: Add a global keybinding that drops into `$EDITOR` on `~/.zshrc`
(and other dotfiles-tracked files), cleanly suspending and resuming the TUI
around the subprocess — the "quick access to .zshrc" requirement (REQ-003).

## Why this phase exists
This phase reuses Phase 3's package/file enumeration (TASK-011) to offer more
than just `~/.zshrc` (e.g. `.zprofile`, `.zshenv`, nvim's `init.lua`, herdr's
`config.toml`) without re-deriving that list, so it must follow Phase 3. It
precedes Upgrades (Phase 5) because editor launch is a simpler, purely local
feature that de-risks the terminal-suspend/resume mechanism before Phase 5
adds network-dependent async work on top of the same event loop.

## Steps
- [x] TASK-014: `src/editor.rs` — `launch_editor(path: &Path)` resolving
      `$EDITOR` (fallback `nvim`, ASSUMPTION-002), suspending the TUI (disable
      raw mode, leave alternate screen via `crossterm::execute!`) before
      spawning the editor as a blocking child process, then re-entering raw
      mode + alternate screen and forcing a full redraw on return.
      Why: CON-004 — the terminal must be handed back to the editor cleanly
      and reclaimed cleanly, or both pinst and the editor render garbled
      output.
- [x] TASK-015: `src/app.rs` — global keybinding (e.g. `e`) dispatched from any
      tab, opening `~/.zshrc` by default; extend to a small picker over the
      other dotfiles-tracked files discovered via Phase 3's package
      enumeration (TASK-011), with the keymap hint shown in the status bar.

## Trade-offs & risks
- ASSUMPTION-002 ($EDITOR set, else fall back to `nvim`) is accepted without a
  full editor-selection UI — out of scope for the stated requirement, which
  only asks for quick access, not editor configuration.
- The editor launch is a blocking child process (not async) by design: while
  the user is editing, there is nothing useful for pinst's event loop to do
  concurrently, so blocking here is simpler than threading it through
  `tokio::select!`.

## Done criteria
- TEST-006: Pressing the editor keybinding on the Overview/Health/Upgrades tab
  opens `$EDITOR` on `~/.zshrc`, and quitting the editor returns cleanly to a
  correctly redrawn pinst TUI with no corrupted terminal state.
