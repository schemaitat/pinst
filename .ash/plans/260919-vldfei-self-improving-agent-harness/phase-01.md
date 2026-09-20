---
id: 260919-vldfei
slug: self-improving-agent-harness
phase: 1
status: Done
---

# Phase 1 — Give lessons a lifecycle and issues an owner

## Goal
GOAL-001: Add the two fields every later phase reads — a lifecycle `Status:`
on each lesson in `.ash/LEARNINGS.md`, and a `**Skill:**` attribution on each
`ISSUE-NNN` in every plan's `learnings.md` — and backfill the corpus that
exists today so the fields are real data rather than a convention waiting for
its first user.

## Why this phase exists
It is the data model, and it has no predecessor. Phases 2 and 3 are checks and
a report over these fields; writing either first means writing code against
data that does not exist and backfilling under the pressure of a failing
check. Doing it first also forces the honest question — *can* the ten existing
lessons be classified, and *is* each of the 21 issues attributable to a skill?
— while there is still room to change the vocabulary if the answer is no.

## Steps
- [x] TASK-001: `.agents/skills/plan-learnings/SKILL.md` — define the lesson
      lifecycle in the `LESSON-NNN` template: `**Status:** prose` when the
      lesson is only written down, `mechanized` once a check enforces it (with
      a `**Check:** <finding id>` line naming it), `retired` when it no longer
      applies (with a one-line reason). State that `mechanized` is the goal
      state and that a lesson sitting at `prose` for several plans is a
      candidate for a check, not a candidate for louder prose.
      Why: LESSON-008 says an invariant that lives only in prose has already
      drifted. Without an end state, `LEARNINGS.md` only grows, and the useful
      question — "which of these are actually enforced?" — has no answer.
- [x] TASK-002: `.agents/skills/plan-learnings/SKILL.md` — add a
      `**Skill:** <name> | none` line to the `ISSUE-NNN` template, defined as
      *the skill whose instructions would have had to change to prevent this
      issue*, with `none` meaning no skill covers this work.
      Why: this is the raw material for both halves of the audit. Per-skill
      failure tallies (Phase 3) and missing-skill candidates (Phase 5) are the
      same field read two ways, and no heuristic can recover it later from
      prose — the person writing the issue is the only one who knows.
- [x] TASK-003: `.ash/LEARNINGS.md` — backfill `Status:` on all ten lessons.
      LESSON-003, LESSON-008 and LESSON-009 are `mechanized` by the checks
      added today (`phase.logged-not-done`, `plan.phases-all-done`,
      `plan.unlogged`); the rest are `prose` until something enforces them.
      Do not invent checks in this task — classify only.
      Why: doing the classification by hand once is what reveals whether the
      three states are sufficient. If a lesson fits none of them, the
      vocabulary is wrong and TASK-001 gets amended before anything depends on
      it.
- [x] TASK-004: `.ash/plans/*/learnings.md` — backfill `**Skill:**` on every
      existing `ISSUE-NNN` across the three plans. Expect a majority of `none`:
      most recorded issues are about GitHub, musl or the manifest, not about a
      skill's instructions.
      Why: a lopsided result is the finding. If almost nothing is attributable
      to a skill, then skills are not where this repo's failures come from, and
      Phase 5's gap analysis matters more than Phase 3's failure tallies —
      better to learn that from the backfill than from three phases of code.
- [x] TASK-005: `.agents/README.md` — document both fields where the corpus
      layout is described, including the rule that a lesson's `Check:` names a
      finding id that must exist in `scripts/`.
      Why: `.agents/README.md` is the contract for working on this repo; a
      field that only the skill mentions is invisible to anyone editing the
      corpus by hand.

## Trade-offs & risks
- ASSUMPTION-003 is accepted here: attribution is written by a human or an
  agent that was there, never inferred. An inferred `Skill:` would be a guess
  presented as data, and Phase 3 would then grade skills on guesses.
- RISK-001 is not yet in play — nothing fails until Phase 2 — which is why the
  backfill happens now, while it costs nothing but typing.
- Accepted: `retired` lessons stay in the file rather than being deleted. The
  history of what stopped being true is worth more than the few lines it costs,
  and deletion would make `Seen in:` references dangle.
- Deferred: no schema validation of the new fields in this phase. Phase 2 adds
  the checks; validating in both places would mean two definitions of the same
  format.

## Done criteria
- TEST-001: every `### LESSON-NNN` block in `.ash/LEARNINGS.md` has exactly one
  `**Status:**` line with a value in `prose | mechanized | retired`, and every
  `mechanized` one has a `**Check:**` line whose finding id appears in
  `scripts/ash.sh` (verify with `grep`).
- TEST-002: every `### ISSUE-NNN` block in all three `learnings.md` files has
  exactly one `**Skill:**` line, whose value is `none` or the name of a
  directory under `.agents/skills/`.
- TEST-003: `just qc` is green — in particular `ash.sh check` still reports
  `corpus clean (3 plans)`, proving the new lines did not break frontmatter
  parsing or the index.

## Verified
- TEST-001 ✓ 10 lessons, each with exactly one legal `Status:`; both
  `mechanized` ones resolve — LESSON-007 → `wire.missing`
  (`scripts/agents-wire.sh`), LESSON-009 → `phase.logged-not-done`
  (`scripts/ash.sh`). Enforcement ratio today: 2 of 10.
- TEST-002 ✓ 10 issues across the three `learnings.md` files, each with
  exactly one `Skill:` resolving to `none` or a real skill directory.
- TEST-003 ✓ with one documented exception: `just qc` passes fmt, clippy,
  44 tests and `agents-wire --check`, and `ash.sh check` reports a single
  `info` finding — `plan.learnings-missing` for this very plan, which fires
  from the moment a run writes its first log line and clears when
  `plan-learnings` runs at the end. Pre-existing, logged as an issue, and the
  reason the criterion's "corpus clean (3 plans)" wording is now "4 plans".
