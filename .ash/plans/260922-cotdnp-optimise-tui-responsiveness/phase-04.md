---
id: 260922-cotdnp
slug: optimise-tui-responsiveness
phase: 4
status: Done
---

# Phase 4 — Safe terminal handoff and background filesystem work

## Goal
**GOAL-004**: ensure editor handoff and harness actions never race for
terminal input, replay stale events, block the event thread on filesystem
classification, or leave displayed health state stale (REQ-004).

## Why this phase exists
This phase depends only on phase 3: it completes lifecycle behavior after all
visible state and navigation transitions have stable redraw semantics. Keeping
terminal ownership and background filesystem work in the final phase avoids
mixing process lifecycle changes into the upgrade pipeline or table-state
work, while leaving one integration boundary to measure at the end.

## Steps
- [x] **TASK-017**: `src/event.rs` — replace the detached terminal reader
  with an owned input-task handle that can be stopped and restarted without
  closing the shared application channel. Probe, health, upgrade, and harness
  producers must remain active while input is paused (RISK-004).
- [x] **TASK-018**: `src/cli/commands/tui.rs`, `src/editor.rs` — after
  TASK-017, pause terminal input before restoring the terminal, launch
  `$EDITOR`, reinitialize and clear the terminal, discard stale terminal
  events, restart input, and turn success/failure into visible app status.
- [x] **TASK-019**: `src/app.rs` — after TASK-018, trigger a background
  Health/config refresh after editor return while retaining the previous
  table until the new result arrives; prevent older health generations from
  replacing a newer refresh.
- [x] **TASK-020**: `src/app.rs`, `src/event.rs`, `src/ui/harness.rs` — after
  TASK-019, represent harness confirmation as calculating or ready, compute
  install/uninstall step counts in `spawn_blocking`, and ignore a stale plan
  result if the selection or requested action changed.
- [x] **TASK-021**: `src/app.rs` and harness state refresh — after TASK-020,
  refresh only the scope changed by a successful harness action and merge it
  with the unchanged scope already in `App`; retain a two-scope startup load.
- [x] **TASK-022**: integration tests and measurement — after TASK-021,
  exercise pause/resume and editor failure with a stub command, prove
  background results survive the handoff, prove harness sizing returns
  asynchronously, rerun the 8-second and 20-second pseudo-terminal timing,
  and run the full quality gate.

## Trade-offs & risks
- The editor remains a synchronous foreground child because it must own the
  user's terminal. The optimization is to suspend competing terminal input,
  not to render behind the editor.
- Background results may arrive while the editor is open. They remain queued
  on the application channel and are coalesced into the first post-editor
  redraw; only terminal-input events are discarded (RISK-004).
- A calculating confirmation adds one intermediate modal state. This is
  preferable to a keypress that appears ignored while filesystem work blocks
  the event handler.
- Refreshing one harness scope assumes an action cannot mutate the other
  scope. Current plans are rooted to one explicit scope; a future cross-scope
  action must opt back into refreshing both.

## Done criteria
- **TEST-008**: while a stub editor owns the terminal, the TUI input task is
  stopped, background application events are retained, stale terminal keys
  are not replayed, and input resumes after both editor success and failure.
- **TEST-009**: returning from the editor starts exactly one health generation
  and the newest result wins when generations complete out of order.
- **TEST-010**: harness plan sizing produces no result until its blocking task
  completes, input remains responsive meanwhile, and a mutation refreshes
  only its selected scope.
- **TEST-011**: compared with the recorded release baseline of roughly 862 ms
  for `pinst list --json`, startup probe wall time does not regress
  materially; a 20-second idle pseudo-terminal run performs no periodic
  application frames. Record commands, build profile, and results in the
  implementation log rather than asserting machine-specific seconds.
- `just qc` is green.
