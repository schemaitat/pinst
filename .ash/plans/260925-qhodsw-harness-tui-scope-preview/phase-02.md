---
id: 260925-qhodsw
slug: harness-tui-scope-preview
phase: 2
status: Proposed
---

# Phase 2 — Safe content preview, scrolling, and states

## Goal
**GOAL-002**: let a person read the selected asset's source and installed
content without leaving the dashboard, safely across empty, missing, binary,
and oversized files, with keyboard scrolling and honest empty/loading states
(REQ-003, REQ-004).

## Why this phase exists
It depends on phase 1's selection-to-row mapping and detail region: the preview
header repeats the source and target paths that phase 1 already computes, and
the preview scroll must reset on the same selection changes phase 1
formalises. It is the payoff slice, and it can be built entirely on the
existing `asset::content` and Docs-pane rendering helpers (GUD-001).

## Steps
- [ ] **TASK-001**: `src/core/harness/asset.rs`, `src/core/harness/preview.rs`,
  `src/core/harness/mod.rs` — after phase 1, add
  `entry_relative(kind, name)` returning `skills/<name>/SKILL.md` or
  `commands/<name>.md`, and a new pure `preview` module with a byte cap,
  `Preview` variants (text with optional truncated size, empty, missing,
  binary, error), and UTF-8-boundary-safe truncation.
  Why: a pure classifier is unit-testable without a terminal and keeps the
  UI a thin renderer (ASSUMPTION-002, ASSUMPTION-003, RISK-003).
- [ ] **TASK-002**: `src/app.rs` — after TASK-001, add `harness_scroll`, a
  method returning the selected `(scope, &AssetStatus)` plus its source
  relative path, and `PgUp`/`PgDn` handling that scrolls the preview;
  reset `harness_scroll` whenever the selection changes and initialise it in
  `App::new`.
  Why: scroll state is navigation state and belongs in the one state machine
  (PAT-001), mirroring `docs_scroll`.
- [ ] **TASK-003**: `src/ui/harness.rs`, `src/ui/statusbar.rs` — after
  TASK-002, add the preview pane: a header with the selected asset's source
  and target paths, a wrapped scrolled `Paragraph` for text, and explicit
  notices for empty, missing, binary, and truncated content; show a clear
  sentence when nothing is selected or the list is empty. Extend the Harness
  status-bar hint with `PgUp/PgDn preview`.
  Why: reusing the Docs pane structure keeps rendering consistent and bounded
  (GUD-001, RISK-001).
- [ ] **TASK-004**: `src/app.rs`, `src/ui/harness.rs` tests — after TASK-003,
  add classifier unit tests (text, empty, NUL binary, oversized truncation)
  and `TestBackend` tests: the selected asset's content renders, moving the
  selection changes the preview, a missing target shows a notice, `PgDn`
  scrolls and a selection change resets the scroll, and the empty state is a
  sentence rather than a blank pane.
  Why: these are the safety properties of REQ-003 and REQ-004 (GUD-002).

## Trade-offs & risks
- Preview bytes are read on demand during draw rather than cached; a 64 KiB
  cap and event-driven redraws bound the cost (RISK-001, ALT-003).
- For `Unmanaged` rows there is no `.agents/` source, so the preview shows the
  target if it is readable and otherwise says the asset is not part of the
  harness.
- Binary detection combines a UTF-8 validity check with a NUL-byte scan; some
  exotic text is misclassified as binary, which is the safe direction for a
  terminal.
- Truncation must not split a multi-byte character, or `String::from_utf8`
  panics on the slice; tests cover the boundary (RISK-003).

## Done criteria
- **TEST-004**: classifier tests cover UTF-8 text, empty bytes, NUL-containing
  bytes as binary, and an over-cap input truncated with a reported byte count.
- **TEST-005**: a `TestBackend` render shows the selected asset's content and
  updates when the selection moves; the preview header shows source and target
  paths.
- **TEST-006**: a `TestBackend` render of a missing target shows a notice,
  not a panic or an empty pane.
- **TEST-007**: `PgDn` increases `harness_scroll` and moving the selection
  resets it to zero.
- **TEST-008**: an empty-but-ready Harness tab renders an explicit "no assets"
  sentence, and the status bar mentions preview scrolling.
- `just qc` is green.
