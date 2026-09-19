---
id: 260919-vldfei
slug: self-improving-agent-harness
phase: 2
status: Proposed
---

# Phase 2 — Make undistilled learnings a finding

## Goal
GOAL-002: Turn "distil regularly" from an instruction nobody is reminded of
into an exit code — an issue that no lesson references and that has not been
declined fails `just qc`, and a lesson claiming to be mechanized by a check
that does not exist fails too.

## Why this phase exists
It depends on Phase 1 alone: these are checks over exactly the two fields
Phase 1 defined and backfilled, and they can only be written once the corpus
passes them. Keeping them out of Phase 3 matters because this phase is the
one that changes behaviour for everyone — it is the first time the harness
will stop a commit over a *judgement* not yet made, rather than over a
malformed file — and that deserves to land on its own, revertable without
taking the audit with it.

## Steps
- [ ] TASK-006: `scripts/ash.sh` — add `learnings.untriaged.<plan>.<issue>`:
      for every `ISSUE-NNN` in a plan's `learnings.md`, a finding unless the
      issue id is referenced by some `**Seen in:**` line in `.ash/LEARNINGS.md`
      or the issue block carries `**Distilled:** declined — <reason>`.
      Severity `warning`; remediation points at the `plan-learnings` skill.
      Why: this is the cadence. A plan closing is what starts the clock, and
      the clock stops when a human or agent has actually decided — promote,
      merge into an existing lesson, or decline. Three outcomes, one of which
      is free, so RISK-001's "this is noise" failure mode has a cheap exit.
- [ ] TASK-007: `scripts/ash.sh` — add `lesson.unenforced.<lesson>`: a lesson
      whose `**Status:** mechanized` names a `**Check:**` id that appears
      nowhere in `scripts/`. Severity `error` — a lesson claiming enforcement
      it does not have is worse than one honestly marked `prose`, because it
      tells the next reader the problem is handled.
      Why: it closes the loop Phase 1 opened. Without it, `mechanized` is a
      label anyone can apply, and the one number this system exists to
      report — how much of what we learned is actually enforced — becomes
      unfalsifiable.
- [ ] TASK-008: `scripts/ash.sh` — extend `check`'s usage text and the JSON
      envelope's `summary` with `lessons` and `issues` counts alongside
      `plans`, so a machine reading the envelope can see corpus size without
      parsing findings.
      Why: PAT-001 — the envelope is the interface. A consumer that has to
      count `### ISSUE-` blocks itself is a consumer that will drift from
      whatever the script means by "issue".
- [ ] TASK-009: negative-test all three behaviours against a copy of the
      corpus under `ASH_DIR`: an untriaged issue fires, the same issue with
      `**Distilled:** declined` does not, and a `mechanized` lesson whose
      `Check:` id is renamed fires as an error. Record the commands in the
      phase file when they pass.
      Why: a check nobody has seen fail is a check nobody knows works —
      exactly how three staleness invariants could have been written and
      quietly never fired. `ASH_DIR` already exists for this.
- [ ] TASK-010: `.agents/README.md` — add the three new findings to the
      documented invariant list, with the same "what it catches" framing as the
      staleness group.
      Why: the list is what someone reads when a finding surprises them, and an
      undocumented finding reads as a bug in the script.

## Trade-offs & risks
- RISK-001 is the live risk of this phase. The mitigation is that `declined`
  is one line and permanent, and that the check is scoped to `ISSUE-NNN`
  blocks — not to sentences, paragraphs or `task_failed` events, which would
  be unbounded.
- Accepted: the check cannot tell a thoughtful decline from a lazy one. It
  enforces that a decision was made and recorded, not that it was a good
  decision. Grading the decision is what the review pass in Phase 5 is for.
- ASSUMPTION-002 is fully committed here: the cadence becomes a property of the
  repo. Reversing it means deleting one finding, not unwinding a design.
- Deferred: no finding for a lesson that has sat at `prose` across many plans.
  It is the interesting signal, but "many" needs a threshold nobody can
  justify yet; Phase 5 reports it instead of failing on it.

## Done criteria
- TEST-004: on a deliberately broken copy of the corpus, `ash.sh check` reports
  `learnings.untriaged.*` for an issue no lesson references, and is silent on
  the same issue once `**Distilled:** declined — <reason>` is added.
- TEST-005: renaming a `**Check:**` id in a copy of `LEARNINGS.md` produces
  `lesson.unenforced.*` at severity `error`; restoring it clears the finding.
- TEST-006: on the real corpus, `just qc` is green — every issue backfilled in
  Phase 1 is either referenced by a lesson or explicitly declined, and every
  `mechanized` lesson names a check that exists.
- TEST-007: `ash.sh check --json` emits `plans`, `lessons` and `issues` in
  `summary`, and remains valid JSON (`python3 -m json.tool`).
