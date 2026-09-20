---
id: 260919-vldfei
slug: self-improving-agent-harness
updated: 2026-09-20
areas: [agents, harness, learnings]
issue_count: 6
---

# Learnings — a self-improving agent harness (260919-vldfei-self-improving-agent-harness)

## Source
- Plan: `./README.md`
- Basis: session context
- Logs consulted: `logs/20260920T060910Z-claude-opus-5.log`

## Summary
All five phases completed in one run: 29 tasks, five commits, no task failed
and no Done criterion missed. The harness now grades its six skills on the
artifacts they leave in the repo, fails `qc` on learnings nobody has triaged,
counts invocations as an opt-in second opinion, and ends every review with an
agenda.

The headline is that the measurements contradicted the plan on their very
first run, twice, and both times the plan was the thing that was wrong. It
predicted which lessons were already mechanized and got two of three wrong
while missing an entire checker; it predicted that issue attribution would be
mostly `none` and it came out 6-of-10 against `plan-write`. Six of ten
recorded failures in this repo implicate the *planning* skill, and none
implicate any other. That is the most useful thing this plan produced, and it
was produced by a backfill that took ten minutes.

## Issues

### ISSUE-001: `qc` is red for the whole duration of every implementation run
**What happened:** The first log line of the run made `just qc` fail, and it
stayed failing until the last. `check_learnings()` emits
`plan.learnings-missing` at `info` as soon as a plan has a `logs/` directory
and no `learnings.md`, and `report()` returns 3 on a finding of *any*
severity.
**Root cause:** The finding fires on a state that is not merely normal but
mandatory — `plan-implement` writes `run_start` before doing anything, and
`learnings.md` cannot exist until the run ends. The check and the lifecycle
that produces its input disagree about when "missing" means "wrong".
**Fix applied:** None — pre-existing, and out of this plan's scope. Worked
around by verifying at each phase boundary that no finding *other* than this
one was present, and requiring a fully green `qc` only after `plan-learnings`.
**Recommendation:** Gate it on the run having ended — `logs/*.log` containing
a `run_end` event — which is the same move as `phase.logged-not-done`:
compare against the record written after the fact. A two-line change, and
until it lands every implementation run has a permanently red gate, which is
how a check earns being ignored.
**Update (2026-09-20):** fixed. `plan.learnings-missing` now requires a `run_end` event in the plan's logs before it fires, so a run in progress is silent and a finished one is not.
**Skill:** none
**Gap:** answered — a defect in a checker, now fixed — the finding gates on run_end

### ISSUE-002: the plan's guess at which lessons were mechanized was wrong two ways
**What happened:** TASK-003 named LESSON-003, LESSON-008 and LESSON-009 as
already mechanized. Doing the classification found LESSON-009 mechanized,
LESSON-003 and LESSON-008 enforced only indirectly, and LESSON-007 —
which the plan never considered — mechanized by `wire.missing`.
**Root cause:** The plan reasoned about which lessons had checks from memory
of having written those checks the previous day, and it reasoned only about
`ash.sh`. `agents-wire.sh` emits findings too, so "the checks live in ash.sh"
was an assumption nobody stated and one `grep -rn finding scripts/` would have
settled. The same blind spot put `scripts/ash.sh` in TEST-001 where TASK-007
correctly says `scripts/`.
**Fix applied:** Classified honestly against what exists — two mechanized, not
three, and a different two. `check_lessons()` searches all of `scripts/`, and
TEST-001 was verified in that corrected form.
**Recommendation:** A plan that asserts a fact about its own repo should carry
the command that establishes it, exactly as it would for a fact about GitHub.
"I wrote that check yesterday" is memory, not evidence, and it is *more*
dangerous than an external assumption because it feels verified.
**Skill:** plan-write

### ISSUE-003: issue attribution came out inverted, and that is the finding
**What happened:** TASK-004 predicted "a majority of `none`: most recorded
issues are about GitHub, musl or the manifest, not about a skill". The
backfill produced `plan-write` 6, `none` 4, and every other skill 0.
**Root cause:** The prediction was made by recalling the *subject matter* of
the issues rather than applying the field's actual definition — the skill
whose instructions would have had to change. Six issues about GitHub
permissions, repo visibility, unsplit Done criteria and handover blocks are
all, under that definition, about how the plan was written.
**Fix applied:** None needed; the data is the deliverable. It reorders the
value of the rest of the plan: per-skill failure tallies matter more than
Phase 3 assumed, and gap analysis less.
**Recommendation:** Define the field first, apply it second, predict never. A
prediction about data that takes ten minutes to collect is a hypothesis with
no purpose — collect it.
**Skill:** plan-write

### ISSUE-004: `SKILL.md` frontmatter has never been valid YAML
**What happened:** Adding `produces:` and `evidence:` prompted a parse check,
and every one of the six skills failed it — on their *existing*
`description:` values, which contain an unquoted colon-space, e.g.
`(type(scope): subject)`.
**Root cause:** Nothing has ever parsed that frontmatter as YAML.
`agents-wire.sh` reads it with `grep` and `awk`, so the file has been
YAML-shaped rather than YAML for its whole life, and the divergence was
invisible by construction.
**Fix applied:** None — pre-existing and out of scope. The new keys are
single-quoted and parse cleanly in isolation.
**Recommendation:** Quote the descriptions (one line per skill), and have
`agents-wire.sh --check` parse the block properly so the claim "this is YAML"
is enforced by something. A format that only ever meets hand-written parsers
is not the format its authors think it is, and the bill arrives the first time
a real parser shows up.
**Update (2026-09-20):** fixed. Every frontmatter value that YAML could not read plain is quoted, and `agents-wire.sh --check` now reports `skill.unquoted-value` so it stays that way.
**Skill:** none
**Gap:** answered — a file-format defect, now fixed and kept fixed by skill.unquoted-value

### ISSUE-005: the invocation column is reflexive — measuring moved the number
**What happened:** The plan's motivating anomaly was `plan-implement` showing
zero invocations while having written a complete run log. Implementing it made
the number 1, because implementing it *was* an invocation of `plan-implement`.
**Root cause:** An invocation count measures the observer as well as the
observed. Artifact conformance does not: reading `git log` does not add a
commit.
**Fix applied:** Verified the original claim by restricting the count to
before 2026-09-20, where it is exactly 0 against a run log from the 19th, and
recorded both numbers in the phase file.
**Recommendation:** Prefer measures that do not move when you look at them.
Where a reflexive one is worth keeping, pin the baseline with a date before
the work starts — otherwise the evidence for the design disappears into the
act of building it.
**Skill:** none
**Gap:** answered — a property of the measure itself, not of any procedure. Carried by LESSON-012

### ISSUE-006: the first agenda had no way to record its own answer
**What happened:** TASK-029 ran the loop once. The agenda asked whether a
skill was missing for the four issues no skill owns; the answer was no, and
there was nowhere to put it. The gap section recomputes from scratch, so it
will ask the same question on every review forever.
**Root cause:** The plan gave issues three outcomes — promote, merge, decline
— and gave gap groups none. Declining was designed as the escape valve for
`learnings.untriaged` (RISK-001) and simply not carried across to the section
that needed it just as much.
**Fix applied:** The answer was written into `phase-05.md`, which is durable
but not something the report can see. The mechanism is deliberately not built.
**Recommendation:** Any recurring report that asks a question needs a place to
record the answer, in the same change that adds the question. Otherwise its
signal decays to noise at exactly the rate people read it — which is the
failure mode this whole plan was written to prevent, reproduced inside the
plan's own last phase.
**Update (2026-09-20):** fixed. An issue can carry `**Gap:** answered — <reason>`, which drops it from the gap report the way `**Distilled:** declined` drops it from triage.
**Skill:** plan-write

## Deviations from the plan, for the record
- **TEST-001 verified in corrected form** (ISSUE-002): the `Check:` id is
  searched for across `scripts/`, per TASK-007, not in `scripts/ash.sh` alone.
- **`check_distillation` refactored in Phase 5**, not Phase 2, once the agenda
  needed the same "which issues are untriaged" answer. One `untriaged_issues()`
  now feeds both the finding and the count.
- **`kind: reference`** added alongside `produces: none` for `pinst`; the plan
  described the exemption but named no field for it.
- **The agenda emits an `info` finding** so `just review` exits 3 when there is
  something to act on. Not in the plan, and the reason `skills` must stay out
  of `qc`.
