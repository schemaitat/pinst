---
id: 260922-cotdnp
slug: optimise-tui-responsiveness
phase: 2
status: Proposed
---

# Phase 2 — Bounded, cache-correct upgrade discovery

## Goal
**GOAL-002**: make user-initiated upgrade checks finish predictably, honor
the cache by default, and end with an explicit state for every displayed tool
without changing CLI update semantics (REQ-002, CON-001, CON-002).

## Why this phase exists
This phase depends only on phase 1: concurrent completions need the visible
state/redraw contract established there. It is kept separate from table
navigation because correctness of cache, timeout, and completion state can be
tested without involving ratatui layout.

## Steps
- [ ] **TASK-005**: `src/app.rs`, `src/ui/statusbar.rs` — introduce explicit
  cache-aware and forced upgrade requests: `r` starts a normal cached check,
  `R` forces lookups, both remain disabled while a run is active, and each
  new run clears or generation-tags prior progress (ASSUMPTION-001).
- [ ] **TASK-006**: `src/core/upgrade/strategies.rs` — after TASK-005, make
  lookup strategies asynchronous, reuse one `reqwest::Client`, and put the
  existing eight-second bound around both HTTP and subprocess completion.
  Keep failure advisory by returning an unavailable result rather than
  failing the whole run.
- [ ] **TASK-007**: `src/core/upgrade/mod.rs` — after TASK-006, split fresh
  cache resolution from remote lookup, run stale/missing lookups with a
  four-operation cap, and return completions through one coordinator that
  alone mutates and saves the cache (ASSUMPTION-002, RISK-001).
- [ ] **TASK-008**: `src/core/upgrade/mod.rs`, CLI update call sites — after
  TASK-007, migrate `check_all` and streaming callers to the asynchronous
  implementation while preserving manifest-order CLI output, comparison
  behavior, cache format, and exit codes (CON-001).
- [ ] **TASK-009**: `src/app.rs`, `src/ui/upgrades.rs` — after TASK-008,
  represent checking, cached, current, upgrade-available, unavailable, and
  unsupported states explicitly; make progress count the run generation
  rather than stale map entries, and ensure completion removes every
  “checking...” label.
- [ ] **TASK-010**: upgrade tests — after TASK-009, use local stubs and
  injected timeouts/cache paths to prove cache-aware versus forced behavior,
  the concurrency cap, timeout degradation, deterministic CLI ordering, and
  complete TUI row states without real network access.

## Trade-offs & risks
- Completion order becomes nondeterministic, but the coordinator and
  `BTreeMap` keep cache ownership and presentation deterministic (RISK-001).
- Four concurrent lookups can still create four subprocesses or HTTP requests
  at once. The cap is deliberately named and tested so evidence can tune it
  later (ASSUMPTION-002).
- Converting strategies to async touches the CLI path as well as the TUI.
  That is accepted to avoid maintaining two implementations of cache and
  version lookup; output order and public behavior remain fixed (CON-001).
- Tests must stub hostile network and command behavior rather than relying on
  the machine's connectivity, following the repository's recorded lesson
  that documented degradation is a test specification.

## Done criteria
- **TEST-003**: a fresh cached entry causes zero strategy calls for `r`, while
  `R` performs a lookup and replaces that cache entry.
- **TEST-004**: controlled lookup tasks demonstrate no more than four active
  operations, subprocess and HTTP timeouts degrade to unavailable, and the
  test suite performs no external network access.
- **TEST-005**: every supported, unsupported, failed, and no-upgrade-strategy
  tool has a terminal display state after `UpgradesDone`; a second run starts
  at zero current-generation completions.
- Existing upgrade comparison, cache, CLI JSON, and exit-code tests pass.
- `just qc` is green.
