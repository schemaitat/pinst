---
argument-hint: [what to plan — the feature, refactor, or change]
description: Write an ADR-style implementation plan into .ash/plans/
---

## Context

- Current branch: !`git branch --show-current`
- Working tree: !`git status --short || echo "(clean)"`
- Existing plans: !`cat .ash/INDEX.md 2>/dev/null || echo "(no .ash/INDEX.md yet — this would be the first plan)"`
- A freshly minted plan id, ready to use: !`scripts/ash.sh new-id`
- Accumulated learnings, to account for *before* choosing an approach: !`cat .ash/LEARNINGS.md 2>/dev/null || echo "(none yet)"`

## Your task

Write a plan for: $ARGUMENTS

Use the `plan-write` skill — read `.agents/skills/plan-write/SKILL.md` and
follow it rather than working from memory, since it carries this repo's
ADR template, its identifier vocabulary, and the linearity rule that
`plan-implement` later depends on.

The id and learnings above are already loaded; don't re-derive them — in
particular, do not mint a second id or derive one by counting plans.

If `$ARGUMENTS` is empty, plan whatever this session has been discussing.
If the session hasn't established enough to plan — no clear goal, or an
approach still genuinely undecided between real alternatives — work that
out with the user first. A plan whose `## Alternatives Considered` was
invented at write-time to fill the section is worse than no plan, because
it reads afterwards like a decision someone actually made.

When you're done, run `just index` to regenerate `.ash/INDEX.md`, then
`just harness` to confirm the corpus is still clean, and report the plan's
id so it can be carried into the commits that implement it.
