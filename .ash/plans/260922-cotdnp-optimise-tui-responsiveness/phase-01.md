---
id: 260922-cotdnp
slug: optimise-tui-responsiveness
phase: 1
status: Done
---

# Phase 1 — Event-driven rendering

## Goal
**GOAL-001**: remove redraws that cannot change the screen and establish an
explicit redraw outcome for every event batch, so later phases can add
asynchronous work without making frame scheduling implicit (REQ-001).

## Why this phase exists
This is the smallest change that removes permanent idle work and gives every
later state transition one place to say whether it is visible. It has no phase
predecessor. Upgrade concurrency, navigation state, and terminal handoff all
build on this redraw contract rather than adding more special cases directly
to the loop.

## Steps
- [x] **TASK-001**: `src/event.rs`, `src/app.rs` — delete `AppEvent::Tick`,
  the 250 ms interval task, and `App::tick_count`.
  Why: the counter has no reader, so the event has no visible meaning
  (RISK-002).
- [x] **TASK-002**: `src/app.rs` — after TASK-001, make event and key
  handling return a small outcome that says whether visible state changed,
  whether an editor launch is pending, and whether the app should quit.
  Preserve resize as a redraw even though it does not mutate `App`.
- [x] **TASK-003**: `src/cli/commands/tui.rs` — after TASK-002, OR redraw
  outcomes while draining the queue, draw once after the batch only when
  needed, retain the unconditional first frame, and restore the terminal
  without an unnecessary final frame after quit.
- [x] **TASK-004**: event-loop and app tests — after TASK-003, prove a no-op
  key event does not request a frame, resize and visible state changes do,
  and a burst of visible events still collapses to one draw.

## Trade-offs & risks
- A future animation will need to introduce its own timed invalidation instead
  of relying on a global tick (RISK-002). That makes the cost attributable to
  the feature using it.
- Ratatui continues to rebuild the active frame and diff terminal cells when
  a redraw is requested. This phase removes needless invocations; it does not
  introduce partial-buffer rendering or widget caches (ALT-004).
- Event outcomes must describe effects rather than event variants: a key
  press can be a no-op in one tab and visible in another.

## Done criteria
- **TEST-001**: a deterministic test observes no periodic application event
  or redraw request while the TUI is idle.
- **TEST-002**: tests cover no-op key, resize, background result, quit, and
  multi-event burst behavior; each requests exactly the expected draw count.
- `just qc` is green.
