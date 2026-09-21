---
argument-hint: [optional steer — an area, a skill, or a lesson to focus on]
description: Run the harness review and act on its agenda
---

## Context

- Review: !`just review 2>&1 || true`
- Lessons: !`grep -c '^### LESSON-' .ash/LEARNINGS.md 2>/dev/null || echo 0` recorded, !`grep -c '^\*\*Status:\*\* mechanized' .ash/LEARNINGS.md 2>/dev/null || echo 0` mechanized
- Plan corpus: !`cat .ash/INDEX.md 2>/dev/null || echo "(no plans yet)"`

## Your task

Work the agenda above, using the `plan-learnings` skill in its corpus-wide
mode — read `.agents/skills/plan-learnings/SKILL.md`, the section "Step 6 —
The corpus-wide pass".

In short: every untriaged issue ends promoted, merged into an existing lesson,
or declined with a reason. Lessons whose enforcement changed get reclassified.
Gap groups get an answer — propose a skill, or say plainly why the work is not
skill-shaped. Propose; never write a new skill unasked.

## Unattended mode

`.github/workflows/distil.yml` runs this same command on a schedule, with no
user in the loop. When there is nobody to propose to and nobody to ask, four
rules apply — the first two are enforced by `just distil-guard verify`, which
runs before anything is published, and the last two are not:

- **Never create a skill.** A proposal is still a proposal with nobody in the
  room. Write it to `.distil-report.md` in the repo root — gitignored, read by
  the workflow into a "Proposed skills — not written" section of the pull
  request — with the trigger description and what it would produce. Editing an
  *existing* skill is allowed, and `just harness` will tell you if you left it
  unwired.
- **Write only inside `.ash/` and `.agents/skills/`.** Nothing else: not
  `src/`, not the workflow, not the manifest.
- **`LEARNINGS.md` is append-only.** A lesson that stopped being true is
  `retired` with a reason, never deleted; the same goes for an issue in a
  plan's `learnings.md`.
- **Leave the corpus clean.** `just harness` must pass on what you produced,
  or the run publishes nothing.

Everything else is unchanged: triage every untriaged issue, reclassify what
the checks now enforce, answer every gap group. The pull request is the review,
so write the reasoning down where a human will read it — a decline with a
one-line reason is a decision someone can disagree with, and a decline with no
reason is not.

$ARGUMENTS
