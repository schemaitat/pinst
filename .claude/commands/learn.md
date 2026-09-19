---
argument-hint: [plan index or slug — omit if you just finished an implementation run]
description: Write up what implementing a plan actually taught us
---

## Context

- Plan corpus: !`cat .ash/INDEX.md 2>/dev/null || echo "(no plans yet)"`
- Plans and whether they have a learnings file yet: !`for d in .ash/plans/*/; do [ -d "$d" ] || continue; printf '%s  %s\n' "$(basename "$d")" "$([ -f "$d/learnings.md" ] && echo 'has learnings.md' || echo 'NO learnings.md')"; done 2>/dev/null || echo "(none)"`
- Implementation logs on disk: !`ls -1 .ash/plans/*/logs/*.log 2>/dev/null || echo "(no run logs — plans implemented before logging, or not yet implemented)"`
- Already-distilled lessons, so nothing gets promoted twice: !`cat .ash/LEARNINGS.md 2>/dev/null || echo "(none yet)"`

## Your task

Write up the learnings for: $ARGUMENTS

Use the `plan-learn` skill — read `.agents/skills/plan-learn/SKILL.md` and
follow it rather than working from memory, since it carries the `ISSUE-NNN`
format, the append-don't-overwrite rule, and the test for what deserves
promoting to `.ash/LEARNINGS.md`.

If you just finished an implementation run in this session, that session
context is the richest source — use it, and cross-check against the run log.
If you're being called standalone for an older plan, read its logs in
chronological order instead.

Write only what the source actually supports. A plan with no logs and no live
session context has no recoverable issues, and the right output there is a
file that says so — not a reconstruction of what probably went wrong. The
value of `.ash/LEARNINGS.md` is that everything in it was actually observed.
