---
id: 260919-vldfei
slug: self-improving-agent-harness
phase: 5
status: Proposed
---

# Phase 5 — Name the gaps and close the loop

## Goal
GOAL-005: Make the report answer the last question — which work keeps being
done with no skill to do it — and give the whole thing a front door: one
command that produces the review agenda, and a documented cadence for acting
on it.

## Why this phase exists
It depends on Phase 4 alone, and is last because a gap is defined by absence:
you can only claim no skill covers some recurring work once you can see what
every existing skill covers and how much of it fires. It is also the phase
that deliberately stops short of enforcement — a missing skill is a judgement
about what is worth automating, and encoding that judgement in a finding would
mean `qc` failing over an opinion.

## Steps
- [ ] TASK-023: `scripts/ash.sh` — add a `gaps` section to the `skills`
      report: issues carrying `**Skill:** none` grouped by the plan `areas`
      they came from, with a count and the issue ids, ordered by frequency.
      Why: this is the direct reading of the field Phase 1 backfilled. Three
      issues in three plans, all `none`, all in the same area, is the shape of
      work that wants a skill — and it is a shape no one notices from inside a
      single plan.
- [ ] TASK-024: `scripts/ash.sh` — add a second gap signal: `prose` lessons
      referenced by issues from two or more distinct plans, listed as
      "recurring, unenforced". No threshold in the code beyond ≥2 plans, and no
      finding.
      Why: Phase 2 deferred exactly this. A lesson that two plans have now hit
      is the strongest available evidence that something is systemic, which
      `plan-learnings` already says about `Seen in:` — this surfaces it instead
      of leaving it to whoever re-reads the file.
- [ ] TASK-025: `scripts/ash.sh` — print a closing `## Agenda` block: untriaged
      issues from Phase 2, skills with no contract, skills whose windowed rate
      is below their all-time rate, the gap groups, and the recurring
      unenforced lessons — each as one line with the command that acts on it.
      Why: a report that ends in numbers gets read; a report that ends in a
      list of next actions gets used. This is the artifact a review session
      starts from.
- [ ] TASK-026: `.claude/commands/distil.md` — a `/distil` entry point that
      runs `just review` and hands the agenda to `plan-learnings` in its new
      corpus-wide mode, rather than pointing it at one plan.
      Why: `.agents/README.md` already treats commands as the entry point per
      lifecycle stage, and distillation has just become a stage of its own
      rather than a tail of implementation.
- [ ] TASK-027: `.agents/skills/plan-learnings/SKILL.md` — document the
      corpus-wide mode: read the agenda, triage every untriaged issue
      (promote, merge, or decline), re-classify lessons whose enforcement
      changed, and propose skill edits or new skills for the gap groups —
      proposing, never writing a skill unasked.
      Why: the skill's existing modes both start from one plan. Without this,
      the agenda has no consumer and the loop stays open at its last inch.
- [ ] TASK-028: `.agents/README.md` — document the cadence: `/distil` at every
      plan close, and whenever `qc` reports `learnings.untriaged`. State
      plainly that the harness has no clock and does not want one, because the
      corpus is the clock.
      Why: ASSUMPTION-002 is the one open decision in this plan; the README is
      where the reasoning goes so that reversing it later is an informed choice
      rather than a rediscovery.
- [ ] TASK-029: run `/distil` once on the real corpus and act on its agenda,
      then record in this file what it found and what changed as a result.
      Why: the only honest proof that a self-improving system improves anything
      is one turn of the loop. If the first agenda is empty or useless, that is
      the finding, and it belongs in this plan's learnings rather than in a
      later surprise.

## Trade-offs & risks
- Accepted, and the central trade-off of this phase: gaps are reported, never
  enforced. A finding would make `qc` fail over "you should probably have a
  skill for this", which is an opinion, and CON-003 means any finding fails the
  build.
- RISK-001 returns in a new form: an agenda long enough to feel hopeless gets
  ignored. Mitigated by ordering strictly by frequency and by TASK-029 running
  it once for real — a first agenda that cannot be cleared in one sitting means
  the thresholds are wrong.
- Accepted: `**Skill:** none` conflates "no skill covers this" with "no skill
  could". Musl toolchain surprises are not a skill-shaped problem. Grouping by
  area rather than counting globally is what keeps that from dominating.
- Deferred: no scheduled CI job opening an issue when the agenda is non-empty.
  It is the obvious next step and deliberately not taken here — ALT-001's
  objection applies to a robot on a timer as much as to a human with a
  calendar, and `qc` already fails on the part that is not a judgement call.

## Done criteria
- TEST-016: `just review` on the real corpus prints all three sections —
  per-skill report, gaps, agenda — exits 3 when the agenda is non-empty and 0
  when it is clear.
- TEST-017: the gap section on today's corpus names at least one group, and
  every issue id it cites resolves to a real `ISSUE-NNN` block carrying
  `**Skill:** none`.
- TEST-018: `/distil` completes one full turn: every untriaged issue ends
  promoted, merged or declined, and `just qc` is green afterwards with no
  `learnings.untriaged` findings left.
- TEST-019: TASK-029's outcome is written into this phase file — what the first
  agenda contained, what was acted on, and whether any skill was proposed —
  so the plan's own learnings have something to compare against.
