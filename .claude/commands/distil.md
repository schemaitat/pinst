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

$ARGUMENTS
