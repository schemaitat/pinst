---
argument-hint: [bounded task to delegate; optionally name agent kind and model]
description: Delegate work through Herdr and supervise it to a verified outcome
---

## Context

- Herdr gate: !`if test "${HERDR_ENV:-}" = 1; then echo "HERDR_ENV=1"; else echo "NOT_IN_HERDR"; fi`
- Herdr client: !`if test "${HERDR_ENV:-}" = 1; then herdr status client 2>&1 || true; else echo "(not queried outside Herdr)"; fi`
- Current pane: !`if test "${HERDR_ENV:-}" = 1; then herdr pane current --current 2>&1 || true; else echo "(not queried outside Herdr)"; fi`
- Agent integrations: !`if test "${HERDR_ENV:-}" = 1; then herdr integration status 2>&1 || true; else echo "(not queried outside Herdr)"; fi`
- Git root: !`git rev-parse --show-toplevel 2>/dev/null || echo "(not a git checkout)"`
- Current branch: !`git branch --show-current 2>/dev/null || echo "(no branch)"`
- Working tree: !`git status --short 2>/dev/null || echo "(not a git checkout)"`

## Your task

Orchestrate: $ARGUMENTS

Read `.agents/skills/orchestrate/SKILL.md` and follow it in full. The context
above is only preflight evidence; the skill owns topology selection, logging,
handoff, supervision, verification, escalation and cleanup safety.

Your role is delegation-only. Do not implement, edit, commit, or help finish
the task in this orchestrator session, and do not co-edit the child's
worktree. Put all deliverable work in prompts to the child; inspect and verify
the result, then nudge, escalate, or abort when acceptance is not met.

If the Herdr gate failed, stop before any Herdr inspection or control. Do not
substitute a background shell or another delegation mechanism: this command is
specifically for an explicitly requested Herdr orchestration.

Do not load a plan index or infer plan-lifecycle ownership. The delegated task
may happen to mention a repository's own process, but orchestration itself is
portable and writes only its `.herdr/orchestrate/` log.
