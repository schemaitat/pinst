---
id: 260918-pzdyxp
slug: dotfiles-manager-tui
phase: 1
status: Done
---

# Phase 1 — Skeleton UI & terminal lifecycle

## Goal
GOAL-001: Boot a ratatui alternate-screen TUI with tab navigation, a status
bar, and a crash-safe terminal lifecycle — no real data yet — so every later
view is added as content, not chrome.

## Why this phase exists
Terminal setup/teardown and panic-safety (RISK-003) must exist before any view
renders real data, otherwise a bug in an early data-fetching view could leave a
developer's terminal corrupted while debugging. This phase has no predecessor;
it establishes the render loop, event loop, and tab chrome (REQ-001's "modern
interface" baseline: rounded borders, consistent palette, hint bar) that
Phases 2–5 build content into.

## Steps
- [x] TASK-001: `Cargo.toml` — init crate `pinst`, add DEP-001 ratatui,
      DEP-002 crossterm, DEP-003 tokio, DEP-004 serde+toml, DEP-005 color-eyre,
      DEP-006 `which`.
- [x] TASK-002: `src/main.rs` — terminal setup (enable raw mode, enter
      alternate screen via crossterm), a panic hook that restores the terminal
      before printing, teardown on normal exit, tokio `#[tokio::main]` entry.
      Why: RISK-003 — must exist before any other code can panic mid-render.
- [x] TASK-003: `src/app.rs` — `App` struct with `active_tab: Tab` enum
      `{Overview, Health, Upgrades}` (PAT-002; the editor quick-access is a
      global action, not a tab — see Phase 4), `should_quit: bool`, stub
      `update()`/`handle_key()`.
- [x] TASK-004: `src/event.rs` — `Event` enum `{Key(KeyEvent), Tick}`; event
      loop reading crossterm events merged with a tick interval via
      `tokio::select!`, feeding `App::handle_key`/`update` (PAT-001/PAT-002
      foundation — later phases add `Event::Probe`/`Event::Health`/
      `Event::Upgrade` variants to this same enum and select loop).
- [x] TASK-005: `src/ui/mod.rs` + `src/ui/theme.rs` + `src/ui/statusbar.rs` —
      top-level `draw()` dispatching the active tab via a `Tabs` widget +
      bordered `Block`s; `theme.rs` defines a small modern palette (accent
      color, rounded border symbols); status bar shows the active tab plus
      keybinding hints (`q` quit, `Tab`/`1-3` switch tabs, placeholder `e` for
      the Phase 4 editor action).

## Trade-offs & risks
- RISK-003 (panic leaving terminal in raw mode) is fully addressed here via the
  panic hook — no shortcut taken.
- ASSUMPTION-001 (Ubuntu/WSL + apt only) is implicitly baked in by not adding
  any platform-detection code; acceptable since it matches bootstrap.sh's own
  scope.

## Done criteria
- TEST-001: `cargo run` launches into an alternate-screen TUI with a visible
  tab bar and status bar; `q` quits cleanly leaving the terminal in its
  original state (verify no leftover raw mode via `stty -a` before/after).
- TEST-002: Forcing a panic (temporary `panic!()`) during render still leaves
  the terminal usable afterward (raw mode disabled, cursor visible) — confirms
  the panic hook works.
