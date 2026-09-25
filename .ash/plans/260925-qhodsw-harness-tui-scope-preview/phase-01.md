---
id: 260925-qhodsw
slug: harness-tui-scope-preview
phase: 1
status: Done
---

# Phase 1 — Group installs per scope with roots, counts, and paths

## Goal
**GOAL-001**: make the project and global install states visually distinct,
each labelled with its install root and per-state counts, and surface every
asset's install path, so the tab answers "what is installed where" without
prose hunting (REQ-001, REQ-002).

## Why this phase exists
The preview pane added in phase 2 needs a row to preview and a place to put
its header; both require the selection-to-row mapping and the per-scope model
this phase establishes. It has no phase predecessor. Doing the grouping first
also delivers the largest readability win on its own, so the tab is already
better if phase 2 is ever cut short.

## Steps
- [x] **TASK-001**: `src/app.rs` — add `pub struct HarnessScopeStat` and
  `pub fn harness_scopes()`/`harness_scope_stat()`, computing installed
  (Linked/Copied), missing, drifted (Drifted/Foreign), and unmanaged counts
  plus the scope root; delete `harness_project_summary`,
  `harness_global_summary`, and `harness_scope_summary`, updating `App::new`
  and the `Harness`/`HarnessScope` event arms.
  Why: one typed source of truth for counts and roots prevents the banner and
  section headers from drifting apart (ASSUMPTION-001, RISK-004).
- [x] **TASK-002**: `src/ui/harness.rs` — after TASK-001, render a scope
  summary line per scope (label, root, `installed/total`, per-state counts)
  and replace the flat table with a grouped `List`: a styled section header
  before each scope's assets, and one line per asset carrying kind, name,
  state, and `target` path. Map `harness_selected` to its display row by
  counting inserted headers, and keep the empty/loading states explicit.
  Why: a `List` groups without column-span limits that a `Table` header row
  cannot satisfy (ALT-001, ALT-002, RISK-002).
- [x] **TASK-003**: `src/app.rs`, `src/ui/harness.rs` tests — after TASK-002,
  add `TestBackend` coverage asserting both section headers with their roots
  and counts render, the target path appears, and the final asset is reachable
  on a short terminal; add a unit test for `harness_scope_stat` counts.
  Why: grouping changes row indices, and only a rendering test proves the
  selection still lands on the intended asset (GUD-002).

## Trade-offs & risks
- The flat `harness_selected` index is retained deliberately so `i`/`u` and
  `move_selection` are untouched (REQ-005). The cost is a draw-time mapping
  from asset index to display index, which RISK-002 covers with tests.
- Removing the summary string fields touches the event arms, but leaves a
  smaller `App`; leaving both models in place would be the drift risk.
- The target path can be long; the list line lets it be clipped by the
  terminal rather than forcing a fixed-width column that wastes space on wide
  paths.

## Done criteria
- **TEST-001**: a unit test proves `harness_scope_stat` reports installed,
  missing, drifted, and unmanaged counts and the scope root.
- **TEST-002**: a `TestBackend` render shows distinct `project` and `global`
  sections, each with its install root and counts, for a seeded two-scope app.
- **TEST-003**: a `TestBackend` render shows a seeded asset's target path, and
  the selected final asset is visible on a short terminal.
- `just qc` is green.
