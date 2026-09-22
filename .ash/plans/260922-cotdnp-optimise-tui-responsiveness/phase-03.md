---
id: 260922-cotdnp
slug: optimise-tui-responsiveness
phase: 3
status: Done
---

# Phase 3 — Navigable and cheaper views

## Goal
**GOAL-003**: make all rendered data reachable on small terminals and remove
the repeated filtering and string allocation that remains on each meaningful
frame (REQ-003).

## Why this phase exists
This phase depends only on phase 2, whose explicit upgrade states give the
Upgrades table stable rows to navigate. Navigation and render-data changes
are one slice because selection, filtering, and viewport behavior must be
clamped and tested together; splitting them would create an intermediate UI
whose state and rows can disagree.

## Steps
- [x] **TASK-011**: `src/app.rs` — add persistent selection for Upgrades and
  both Health panes, a Health pane focus, and a Docs page scroll offset.
  Reset or clamp each value when search, tab data, or selected document
  changes (RISK-003).
- [x] **TASK-012**: `src/app.rs`, `src/ui/health.rs`,
  `src/ui/upgrades.rs`, `src/ui/statusbar.rs` — after TASK-011, route
  `j`/`k`, arrows, Tab or pane-focus keys, and PageUp/PageDown to the active
  table; render Health and Upgrades with persistent `TableState`; make hints
  tab-specific so every advertised key acts.
- [x] **TASK-013**: `src/app.rs`, `src/ui/docs.rs` — after TASK-012, keep
  `j`/`k` for the Docs tool list, add explicit page scrolling, reset scroll
  when the selected tool or query changes, and apply the offset through
  `Paragraph::scroll`.
- [x] **TASK-014**: `src/ui/docs.rs`, `src/app.rs` — after TASK-013, compute
  the ranked Docs result once per frame and pass it to both list and reading
  panes; avoid the third ranked search on navigation by exposing the current
  result length without rebuilding matching recipes the TUI discards.
- [x] **TASK-015**: `src/ui/overview.rs`, `src/ui/health.rs`,
  `src/ui/upgrades.rs`, `src/ui/docs.rs`, `src/ui/harness.rs`,
  `src/ui/mod.rs`, `src/ui/statusbar.rs` — after TASK-014, borrow stable
  strings in cells and spans where ratatui lifetimes permit, avoid cloning
  upgrade versions twice, and compute fixed tab titles and harness summary
  data once per state change. Do not add a general widget cache (GUD-001).
- [x] **TASK-016**: rendering tests — after TASK-015, exercise every view in
  a short `TestBackend`, navigating to the final row and scrolling a long
  Docs page; assert selection remains valid after filtering and data refresh.

## Trade-offs & risks
- Health needs both a selected row and an active pane. A visible focus style
  is required so navigation never silently targets the other table
  (RISK-003).
- Building all table rows remains acceptable at the current scale; ratatui
  owns clipping and terminal diffing. Visible-row virtualization and complete
  widget caching stay deferred under GUD-001 and ALT-004.
- Borrowing strings reduces allocations but can make widget lifetimes more
  explicit. Prefer a small retained clone over introducing self-referential
  state or unsafe code.
- Ranked Docs search keeps the CLI's scoring rules. The TUI may skip recipe
  collection, but it must not implement a second ranking algorithm.

## Done criteria
- **TEST-006**: at a 100×15 test terminal, keyboard navigation can reveal and
  select the final Overview, Health, Upgrades, Docs-list, and Harness row;
  filtering to shorter or empty results never leaves an invalid selection.
- **TEST-007**: a Docs page longer than its pane scrolls to its final line,
  resets on tool/query change, and the list ranking matches core Docs search.
- An allocation-oriented code review confirms Docs ranking runs once per
  rendered Docs frame and stable row strings are borrowed where practical.
- `just qc` is green.
