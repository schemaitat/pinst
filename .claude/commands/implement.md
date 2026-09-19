---
argument-hint: [plan id or slug, e.g. 260919-qwerty — omit to resume the one in progress]
description: Implement a plan from .ash/plans/, phase by phase, with a run log
---

## Context

- Current branch: !`git branch --show-current`
- Working tree: !`git status --short || echo "(clean)"`
- Plan corpus: !`cat .ash/INDEX.md 2>/dev/null || echo "(no plans yet — use /plan first)"`
- Phase status across all plans: !`grep -H '^status:' .ash/plans/*/phase-*.md 2>/dev/null | sed 's|\.ash/plans/||' | sort || echo "(no phase files)"`
- Harness state: !`scripts/ash.sh check 2>&1 || true`

## Your task

Implement: $ARGUMENTS

Use the `plan-implement` skill — read
`.agents/skills/plan-implement/SKILL.md` and follow it rather than working
from memory, since it carries the run-log format, the resume rules, and the
per-phase commit and changelog protocol.

The corpus and phase statuses above are already loaded. Use them to pick the
resume point instead of re-listing the directory — but still read the plan's
own `README.md` and the current `phase-NN.md` in full before touching code.
The index is a routing table, not a substitute for the decision record.

If `$ARGUMENTS` is empty and exactly one plan is `In Progress`, resume it. If
it's empty and several plans are candidates, ask which one — picking an
unfinished plan to continue is not a silent call.

Stop and ask rather than improvising if an `ASSUMPTION-NNN` turns out false
or the codebase has drifted from what the plan assumed. The plan exists
because the *what* and *why* were already settled; quietly deciding
otherwise mid-implementation is the one failure this whole lifecycle is
built to prevent.
