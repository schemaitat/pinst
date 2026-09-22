---
id: 260922-cotdnp
slug: optimise-tui-responsiveness
status: In Progress
created: 2026-09-22
updated: 2026-09-22
areas: [tui, events, upgrades, harness]
summary: Make pinst's TUI event-driven, responsive during slow work, and fully navigable without changing its command-line contract.
files_touched: [src/event.rs, src/cli/commands/tui.rs, src/app.rs, src/editor.rs, src/core/upgrade/mod.rs, src/core/upgrade/strategies.rs, src/ui/mod.rs, src/ui/overview.rs, src/ui/health.rs, src/ui/upgrades.rs, src/ui/docs.rs, src/ui/harness.rs, src/ui/statusbar.rs]
---

# Make the TUI responsive and scalable

## Context
The dashboard is already structured around one `App` state machine and one
event queue, and it correctly moves probe, diagnosis, upgrade, and harness
work away from the rendering thread. The remaining costs come from policy at
the edges: an unused 250 ms tick causes four complete frame computations per
second, the Upgrades tab bypasses its own 24-hour cache and performs remote
lookups serially, and several tables and document pages render more data than
the user can navigate (REQ-001, REQ-002, REQ-003).

Terminal handoff and harness confirmation expose a second class of issue.
While `$EDITOR` owns the terminal, the input task remains alive and events
continue accumulating; after it closes, Health still describes the old file
state. Computing a harness confirmation count also performs filesystem work
synchronously in the event handler (REQ-004).

Release measurements put `pinst list --json` at about 862 ms per run on this
machine: 26 detection commands followed by 24 version commands for installed
tools. By comparison, manifest/catalogue loading is bounded at roughly 4.9 ms
above process startup, two-scope harness status at roughly 1.8 ms, and config
status at roughly 1.8 ms. Startup probing therefore dominates current wall
time. The unused idle tick still schedules about 14,400 frame computations per
hour; ratatui may suppress unchanged terminal writes, but row, vector, filter,
and string construction still runs. The implementation should remove proven
waste without replacing the small state machine with a framework or a general
incremental renderer (GUD-001).

## Decision
Keep the existing single `AppEvent` stream and ratatui full-frame drawing
model, but make drawing event-driven: remove the unused tick and let event
handling report whether visible state changed. Then make upgrade discovery
an asynchronous, bounded pipeline with one shared HTTP client, explicit
timeouts, cache-aware and forced user actions, and a result for every row.

Add persistent navigation state for Health and Upgrades plus an independent
scroll offset for the Docs reading pane. Render paths will share one filtered
result per frame and borrow stable strings where practical; broad widget
caching is deferred until measurements show it is needed. Finally, give the
terminal input task an explicit pause/resume lifecycle around `$EDITOR`,
refresh Health on return, and move harness plan sizing off the event thread.

The work retains the existing Tokio, crossterm, ratatui, and reqwest stack
(DEP-001), preserves the CLI's JSON and exit-code behavior (CON-001), and
keeps upgrade network access user-initiated rather than running on startup or
tab selection (CON-002). A normal `r` uses the 24-hour cache and `R` explicitly
forces remote lookup (ASSUMPTION-001); the first bounded-concurrency limit is
four lookups (ASSUMPTION-002).

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Keep the 250 ms tick and only make it slower | No view reads `tick_count`, so any cadence still spends work on no visible transition and continues queueing events during editor sessions |
| ALT-002: Debounce redraws but leave upgrade checks serial | It reduces frame count while leaving the largest user-visible wait unchanged: remote and subprocess lookups still accumulate wall time one after another |
| ALT-003: Run every upgrade lookup concurrently | Fast on a developer machine but creates avoidable process spikes and remote rate-limit pressure; a small cap gives most of the latency win with predictable load |
| ALT-004: Cache complete ratatui widgets and buffers | Adds invalidation complexity across probe, search, resize, selection, and background-result events before current measurements justify it; shared filtered data and borrowed strings capture the obvious wins first |
| ALT-005: Replace the event loop or TUI framework | The current queue already batches bursts and isolates background work. The problems are local lifecycle and scheduling choices, not a limitation of ratatui or Tokio |

## Consequences
- **REQ-001**: idle sessions stop rebuilding unchanged frames, while resize,
  input, and background results still redraw promptly.
- **REQ-002**: upgrade refresh uses cache by default, has bounded wall-clock
  behavior per lookup, and distinguishes checking, unavailable, unsupported,
  and complete states.
- **REQ-003**: every table row and long Docs page becomes reachable from the
  keyboard, and the status bar advertises only keys active on the current tab.
- **REQ-004**: editor handoff cannot consume or replay stale TUI input, Health
  refreshes after edits, and harness plan sizing no longer pauses input.
- **DEP-001**: implementation uses the existing ratatui, crossterm, Tokio,
  and reqwest dependencies; no executor, cache, or terminal abstraction is
  added.
- **CON-001**: core upgrade internals may become async, but `pinst update`,
  JSON envelopes, exit codes, cache location, and comparison semantics stay
  unchanged.
- **CON-002**: opening the Upgrades tab remains side-effect free. Only `r` or
  `R` starts checks.
- **GUD-001**: optimize measured or directly evidenced paths first. Do not add
  wholesale widget caching, lowercase shadow models, or probe throttling
  without a benchmark showing they matter after these changes.
- **RISK-001**: bounded concurrent results arrive out of manifest order and
  can race cache updates. A single coordinator owns cache mutation and emits
  results; the UI's `BTreeMap` remains the ordering boundary.
- **RISK-002**: removing the tick means a future spinner cannot animate until
  a timed invalidation is deliberately reintroduced. This is accepted because
  no spinner exists and a future animation should own its cadence explicitly.
- **RISK-003**: more selection and scroll state creates reset/clamping cases
  on search, resize, and data refresh. State transitions and small-terminal
  rendering tests cover those cases.
- **RISK-004**: pausing terminal input must not pause probe, upgrade, health,
  or harness result delivery. Only the crossterm reader is stopped; the shared
  application channel remains alive.
- **ASSUMPTION-001**: lowercase `r` means cache-aware refresh and uppercase
  `R` means force. This makes the existing “cached for 24h” copy truthful and
  keeps forced access explicit.
- **ASSUMPTION-002**: four concurrent upgrade lookups is the initial balance
  between latency, subprocess pressure, and remote rate limits. Keep it a
  named constant so measurements can change it without redesign.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Event-driven rendering | [phase-01.md](phase-01.md) | Done |
| 2 | Bounded, cache-correct upgrade discovery | [phase-02.md](phase-02.md) | Done |
| 3 | Navigable and cheaper views | [phase-03.md](phase-03.md) | Proposed |
| 4 | Safe terminal handoff and background filesystem work | [phase-04.md](phase-04.md) | Proposed |

## Affected Files
- **FILE-001** `src/event.rs` — remove the unused periodic tick and expose
  lifecycle control for the terminal-input producer.
- **FILE-002** `src/cli/commands/tui.rs` — draw only after visible changes,
  coordinate editor handoff, and preserve burst draining.
- **FILE-003** `src/app.rs` — remove tick state; add redraw outcomes,
  navigation/scroll state, upgrade request state, editor return handling, and
  asynchronous harness-plan events.
- **FILE-004** `src/editor.rs` — return actionable launch outcomes while
  retaining terminal restoration and reinitialization.
- **FILE-005** `src/core/upgrade/mod.rs` — make cache lookup and result
  coordination safe under bounded concurrency.
- **FILE-006** `src/core/upgrade/strategies.rs` — share the HTTP client and
  apply timeouts to network and subprocess strategies.
- **FILE-007** `src/ui/mod.rs`, `src/ui/overview.rs`,
  `src/ui/health.rs`, `src/ui/upgrades.rs`, `src/ui/docs.rs`,
  `src/ui/harness.rs`, `src/ui/statusbar.rs` — persistent navigation,
  accurate state labels and hints, one-pass filtered data, scrolling, and
  avoidable clone removal.

## Open Questions
None. ASSUMPTION-001 and ASSUMPTION-002 are explicit defaults with tests and
named constants, so implementation can revise their values without changing
the architecture.
