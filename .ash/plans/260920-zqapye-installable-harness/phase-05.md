---
id: 260920-zqapye
slug: installable-harness
phase: 5
status: Proposed
---

# Phase 5 — The Harness tab

## Goal
Give the dashboard a fifth tab that answers "where is my harness installed"
without reading anything, and lets the answer be changed from there.

## Why this phase exists
It comes after the picker because both of them need the same thing from the
core — a selection turned into a plan — and the picker is where that shape was
settled. Doing the tab first would mean inventing a second way to express "these
skills, that scope" inside `App`, and then reconciling the two.

It is the last phase that adds anything, deliberately: it is the only one whose
value is entirely presentational, so if the plan runs out of room, it is the one
to lose. Everything before it is reachable from the command line.

## Steps

- [ ] TASK-026: `src/app.rs` — `Tab::Harness` as the fifth entry in `Tab::ALL`,
      reachable by `5` and by cycling; `harness: Vec<AssetStatus>` covering both
      scopes, `harness_ready: bool`, `harness_selected: usize`, and
      `filtered_harness()` so `/` filters it the way it filters every other tab.
- [ ] TASK-027: `src/event.rs` + `src/app.rs` — load the harness state in
      `spawn_blocking` and deliver it as an `AppEvent`, exactly as the health
      check does.
      **Why off the runtime:** classification stats every managed path and reads
      every copied file to compare it; that belongs off the async worker threads,
      and `spawn_health_check` is the pattern already there (PAT-003).
- [ ] TASK-028: `src/ui/harness.rs` — a banner of one line per scope naming the
      directory, whether a receipt exists, the link style, and the counts
      (`project  /repo/.claude   6 skills, 6 commands, linked`  /  `global
      ~/.claude   not installed`), above a table of assets with their state
      coloured through `theme` the way `health` colours severities.
      **Why the banner is two fixed lines rather than a list:** "where is it
      installed" has exactly two possible answers on this machine, and a reader
      should get both without moving the cursor (REQ-004, REQ-005).
- [ ] TASK-029: `src/app.rs` + `src/ui/mod.rs` — `i` installs and `u`
      uninstalls the selected scope, both behind a confirmation modal reusing
      `draw_picker`'s centred-rect treatment and spelling out the step count;
      the run goes through the same `build_install_plan` / `build_uninstall_plan`
      and `Runner` as the CLI, in `spawn_blocking`, with the resulting state
      refreshed on completion.
      **Why the same path and not a shortcut:** two ways to mutate is two places
      for `--dry-run` and the blocked-on-unmanaged rule to be wrong, and the TUI
      is the one with no `--dry-run` to fall back on (CON-002, SEC-001).
- [ ] TASK-030: `src/ui/statusbar.rs` — the hint line gains `[i] install  [u]
      uninstall` while the tab is active; tests — a `TestBackend` render asserts
      the banner names both scopes and that a skill name and its state reach the
      screen, in the shape of the existing docs-tab render test; the confirmation
      modal blocks `i` from acting until Enter.

## Trade-offs & risks
The tab mutates, which no tab did before — `e` opens an editor and everything
else is read-only. The confirmation modal is the whole mitigation, and it is a
weaker gate than the CLI's, which has `--dry-run` and an explicit `--yes`.
Naming the step count in the modal is what keeps it honest: "install 12 items
into /repo/.claude" is a sentence someone can disagree with, where "install?"
is not.

Refreshing after a run re-runs the classification rather than mutating the
state in place. Slower and correct; the alternative is a view that believes its
own plan rather than the filesystem, which is the failure `pinst config status`
exists to catch elsewhere.

RISK-006 applies here with less protection than on the CLI: a global install
from the tab can encounter the user's personal skills. The blocked-on-unmanaged
rule is enforced in the plan builder, not the UI, so it holds — but the operator
sees it as a blocked row rather than as a message they had to acknowledge.

## Done criteria
- TEST-019: `cargo test` renders the tab with `TestBackend` and asserts both
  scope lines and at least one asset row with its state.
- TEST-020: the tab is reachable by `5`, by `Tab` cycling, and `/` filters its
  rows without moving the other tabs' cursors.
- TEST-021: `i` on a scope with nothing installed opens a modal naming the step
  count; Esc closes it having changed nothing; Enter installs and the banner
  flips to installed on the next frame.
- TEST-022: `u` on an installed scope removes exactly the receipt's entries, and
  a row in `Unmanaged` state stays on screen and on disk.
