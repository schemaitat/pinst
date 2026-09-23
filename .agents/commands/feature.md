---
argument-hint: [feature request and acceptance criteria]
description: Implement a feature end to end in an isolated worktree, from plan to pull request
---

## Context

- Current checkout: !`git rev-parse --show-toplevel 2>/dev/null || echo "(not a git checkout)"`
- Current branch: !`git branch --show-current 2>/dev/null || echo "(no branch)"`
- Working tree: !`git status --short 2>/dev/null || echo "(not a git checkout)"`
- Committed base: !`git rev-parse HEAD 2>/dev/null || echo "(no committed base)"`
- Existing plans: !`cat .ash/INDEX.md 2>/dev/null || echo "(no plan index)"`
- Quality gate: `just qc`

## Your task

Implement this feature end to end: $ARGUMENTS

Read `.agents/skills/implement-feature/SKILL.md` and follow it in full. It
composes the existing `plan-write`, `plan-implement`, `plan-learnings`, and
`create-pr` skills; do not replace their plan format, run logs, phase commits,
or PR body rules.

Start from a safe isolated worktree based on a committed ref. Persist and
record the exact plan id before implementation. Execute every phase in order,
run the acceptance checks and `just qc`, and create a PR only when publication
is authorized. Preserve user decisions and stop for dirty-base choices,
product decisions, credentials, trust, permissions, or failed pre-existing
checks. Report the worktree/branch, plan path, phases, verification, PR URL
when any, and unresolved blockers. A delegated run is not complete merely
because its agent becomes idle or a terminal state is observed.
