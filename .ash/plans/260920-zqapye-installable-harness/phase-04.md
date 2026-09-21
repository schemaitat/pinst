---
id: 260920-zqapye
slug: installable-harness
phase: 4
status: Done
---

# Phase 4 — The interactive picker

## Goal
Make a bare `pinst harness install` at a terminal ask the three questions it
needs answered — which scope, which skills, which commands — and turn the
answers into exactly the flags Phase 2 already implements.

## Why this phase exists
It follows the flags rather than preceding them because the picker is a front
end over a selection, and a selection was not a defined thing until Phase 2
defined it. Building the UI first would have meant designing a dialog for
arguments that did not yet exist, and then changing it when they did.

It is separate from Phase 5 even though both are terminal UI, because they have
opposite contracts. The picker is a short-lived modal that must never appear in
`--json` mode or on a pipe — a prompt on stdout is how an agent deadlocks — while
the tab is a view inside a dashboard that is already interactive by definition.
Sharing a phase would mean one set of tests for two different rules about when
it is legal to ask a human anything.

## Steps

- [x] TASK-022: `src/prompt.rs` — a selection state machine with no terminal in
      it: `Selection { scope, skills: Vec<(String, bool)>, commands: Vec<(String,
      bool)> }`, a `Step` cursor over scope → skills → commands → confirm, and
      key handling as pure functions over that state.
      **Why pure first:** the awkward alternative is testing a picker by driving
      a terminal, which is the tell that the state wants to be a parameter
      rather than ambient (LESSON-018).
- [x] TASK-023: `src/prompt.rs` — the ratatui rendering over that state, entered
      with `ratatui::init()` and left with `ratatui::restore()`, the way
      `editor::launch` already suspends and resumes the dashboard. Space toggles,
      `a` selects all, Enter advances, Esc cancels the whole run.
      **Why not a prompt crate:** the release profile is tuned for size and
      `ratatui` and `crossterm` are already linked in for the dashboard; a
      ~120-line selector over primitives already paid for beats a dependency that
      re-implements them (ALT-006, CON-003).
- [x] TASK-024: `src/cli/commands/harness.rs` — call the picker from `install`
      only when `ctx.interactive()` is true *and* no selection flag was given;
      pre-tick everything already installed at the chosen scope so the first
      screen shows the current state rather than an empty form; a cancel exits
      `2` having changed nothing. In `--json` mode, on a pipe, or with any
      selection flag, behave exactly as Phase 2 does today.
      **Why pre-ticked:** the question a returning user is answering is "what
      should be installed", not "what would you like to add", and those differ
      the moment anything is already there.
- [x] TASK-025: tests — the state machine, driven by key codes, produces the
      expected selection; `ctx.interactive()` is false under `--json` and on a
      non-TTY, so the picker is unreachable there; an explicit `--skill` skips
      the picker entirely.
      **Why the second one is a test and not a comment:** `Ctx::interactive`
      already encodes this rule for `confirm`, and a second caller getting it
      wrong is a hang rather than an error — the worst shape of bug for an agent
      to debug from the other side of a pipe.

## Trade-offs & risks
The picker is the only place in pinst that takes over the terminal outside the
dashboard, and the failure mode of getting the guard wrong is not a wrong
answer but a hang. That is why the guard is `ctx.interactive()` — the existing
predicate, already used by `confirm`, already tested — rather than a new check
of `stdout().is_terminal()` written at the call site.

Uninstall does **not** get a picker in this phase. It could reuse the same
state machine over the receipt's entries, and probably should later, but the
common case is `--all` and the risk of an accidental Enter is asymmetric
between the two commands. Deferring it costs nothing; adding it later is the
same 20 lines.

Selection is per-run and is not remembered. A "profile" of chosen skills would
be a fourth thing to keep in sync with the receipt and the source tree, and the
receipt already answers what is installed.

## Done criteria
- TEST-015: `cargo test` drives the selection state machine through scope,
  skills, commands and confirm, and through a cancel at each step.
- TEST-016: `pinst harness install --json` and `pinst harness install < /dev/null`
  never prompt and behave as the flag-driven path.
- TEST-017: `pinst harness install` at a terminal, with nothing installed,
  presents all six skills and six commands unticked; run again after installing
  three, it presents those three ticked.
- TEST-018: cancelling with Esc exits `2` and leaves the tree unchanged.
