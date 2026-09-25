---
id: 260925-qhodsw
slug: harness-tui-scope-preview
status: Proposed
created: 2026-09-25
updated: 2026-09-25
areas: [tui, harness, app]
summary: Make the Harness tab show project and global installs as distinct, path-and-count-annotated sections with a safe content preview of the selected asset and keyboard scrolling.
files_touched: [src/app.rs, src/ui/harness.rs, src/ui/statusbar.rs, src/core/harness/asset.rs, src/core/harness/preview.rs, src/core/harness/mod.rs]
---

# Make the Harness tab scannable and inspectable

## Context
The Harness tab is the only place a person can see, at once, how the agent
harness is installed at both scopes, yet it spends its space poorly. It renders
a two-line banner followed by a single flat table of `Scope | Kind | Name |
State`; project and global rows are interleaved with only a four-character
scope tag to tell them apart, the install root is buried in prose, and the
per-asset install path (`AssetStatus.target`, built by `state::target_path`) is
never shown anywhere. There is no way to see what an asset actually *is*
without leaving the TUI for `cat` (REQ-001, REQ-002, REQ-003).

The Docs tab already demonstrates the shape the Harness tab is missing: a list
on the left, a wrapped, scrollable `Paragraph` reading pane on the right, and
theme-driven styles. `.agents/` content is already readable through
`harness::asset::content`, and both scopes are already classified into
`Vec<AssetStatus>` in `App`. The work is therefore a rendering and small
state-model change, not a new data source (GUD-001).

The install/uninstall confirmation flow (`i`/`u`, `HarnessModal`, and the
spawned plan/execute tasks) is correct and must keep working unchanged; it
already reads the selected row's scope out of `filtered_harness()`, so the
grouping must preserve that flat selection index (REQ-005).

## Decision
Render the Harness tab as three cooperating regions: a scope summary showing
each scope's label, install root, and per-state counts; a single scrollable
asset list grouped into styled per-scope section headers; and a detail/preview
pane for the selected row. The one list keeps `harness_selected` as an index
into the same flat `filtered_harness()` order the modal already consumes, so
`i`/`u` behaviour is untouched; section headers are display-only lines inserted
around the rows and selection is mapped to a display index at draw time
(ALT-001, ALT-002).

Each asset line carries its kind, name, state, and target path, and the
selected asset's source and target paths are repeated in the preview header so
a path is always visible even on narrow terminals. The preview reads the
source entry (`skills/<name>/SKILL.md` or `commands/<name>.md`) and the
installed target through the existing `asset::content` / filesystem helpers,
classifies the bytes through a small pure `core::harness::preview` function,
and truncates oversized content at a UTF-8 boundary with an explicit notice
instead of dumping a binary or a multi-megabyte file into the terminal
(REQ-002, REQ-003, RISK-003, ASSUMPTION-002).

Readability work is deliberately local: `PgUp`/`PgDn` scroll the preview,
selection changes reset that scroll (mirroring `docs_scroll`), and empty and
loading states get real sentences. No new dependencies are introduced
(CON-002), and the existing summary string fields are replaced by a typed
`HarnessScopeStat` so counts and roots have one source of truth (ASSUMPTION-001).

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Keep `Table` and insert spanning section-header rows | ratatui table cells cannot span columns, so a header row would be clipped into the first column; a `List` of formatted lines groups cleanly and matches the Docs pane |
| ALT-002: Two side-by-side per-scope tables | A single flat selection index is what `open_harness_modal` and `move_selection` rely on; two tables would require a second selection dimension for no rendering gain |
| ALT-003: Cache preview bytes in `App` on selection change | Adds a second invalidation path alongside install/uninstall refreshes; drawing is already event-driven, and the 64 KiB cap bounds on-demand reads |
| ALT-004: Shell out to `bat`/`less` for content | Introduces an external dependency and leaves the ratatui frame; the in-pane `Paragraph` matches the Docs tab and needs nothing installed |
| ALT-005: Leave the flat table and only add a detail pane | Fails REQ-001: project and global rows remain visually indistinguishable and the install root stays hidden |

## Consequences
- **REQ-001**: project and global installs become distinct sections, each with
  its root path and installed/total plus per-state counts.
- **REQ-002**: the target path appears on every asset line and in the selected
  detail header.
- **REQ-003**: the selected asset's source and installed content are shown
  safely, with explicit empty, missing, binary, and truncated states.
- **REQ-004**: long lists and previews scroll; loading and empty states are
  explicit; the status bar advertises the preview keys.
- **REQ-005**: `harness_selected` keeps indexing `filtered_harness()` and the
  modal flow is unchanged.
- **CON-001**: only the files in `## Affected Files` change; no unrelated
  refactors.
- **CON-002**: no new crates; ratatui, crossterm, and the existing `theme`
  module only (DEP-001).
- **GUD-001**: the reading-pane structure is copied from `src/ui/docs.rs`.
- **GUD-002**: new coverage is inline `#[cfg(test)]` with ratatui
  `TestBackend`, the pattern `src/app.rs` already uses.
- **PAT-001**: views stay pure functions of `App`; all mutation continues to
  flow through `handle_event`.
- **RISK-001**: reading files during draw could stall on large assets; the
  preview caps at `MAX_PREVIEW_BYTES` (ASSUMPTION-003).
- **RISK-002**: header lines shift display rows away from asset indices;
  selection is mapped through the inserted row count and tests assert the
  final row remains reachable.
- **RISK-004**: removing `harness_project_summary`/`harness_global_summary`
  could break another caller; a repo-wide grep confirms the banner is the only
  reader, and the fields are replaced in the same change.
- **DEP-001**: relies only on `ratatui`, `crossterm`, and existing core
  helpers (`asset::content`, `state::target_path`, `Source`).

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Group installs per scope with roots, counts, and paths | [phase-01.md](phase-01.md) | Done |
| 2 | Safe content preview, scrolling, and states | [phase-02.md](phase-02.md) | Proposed |

## Affected Files
- **FILE-001** `src/app.rs` — add `HarnessScopeStat` and `harness_scopes()`/
  `harness_scope_stat()`, replace the two summary-string fields and
  `harness_scope_summary`, and add preview selection helpers plus
  `harness_scroll` state and key handling.
- **FILE-002** `src/ui/harness.rs` — rewrite the draw layout into scope
  summary, grouped asset list, and selected detail/preview pane; keep
  `draw_modal` intact.
- **FILE-003** `src/ui/statusbar.rs` — advertise `PgUp`/`PgDn` preview
  scrolling on the Harness tab.
- **FILE-004** `src/core/harness/asset.rs` — add `entry_relative(kind, name)`
  for deriving an asset's `SKILL.md`/command path from its recorded kind and
  name.
- **FILE-005** `src/core/harness/preview.rs` — new pure classifier for preview
  bytes: text, empty, missing, binary, and truncated with a byte cap.
- **FILE-006** `src/core/harness/mod.rs` — register the `preview` module.

## Open Questions
None. ASSUMPTION-001 through ASSUMPTION-003 are explicit defaults with tests;
the truncation cap is a named constant so it can be retuned without redesign.
