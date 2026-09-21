---
argument-hint: [optional steer — draft, a title, or what to emphasise]
description: Open a pull request built from the plan the branch implements
---

## Context

- Current branch: !`git branch --show-current`
- Working tree: !`git status --short || echo "(clean)"`
- Commits on this branch: !`git log --oneline main..HEAD 2>/dev/null || echo "(none — nothing to open a PR for)"`
- Behind main by: !`git rev-list --count HEAD..main 2>/dev/null || echo "?"` commit(s)
- Plan(s) this branch claims: !`git log main..HEAD --format=%B 2>/dev/null | grep '^Plan: ' | sort -u | grep . || echo "(none — no Plan: footers)"`
- Existing PR for this branch: !`gh pr view --json number,title,isDraft,url 2>/dev/null || echo "(none)"`
- Type/scope vocabulary: !`git log --format=%s -60 | grep -oE '^[a-z]+(\([a-zA-Z0-9_.,/ -]+\))?' | sort | uniq -c | sort -rn | head -12`

## Your task

Open a pull request for this branch.

Use the `create-pr` skill — read `.agents/skills/create-pr/SKILL.md` and
follow it rather than working from memory, since it carries this repo's
release-please constraint on PR titles and the mapping from a plan's ADR
sections to the PR body.

User's steer (may be empty): $ARGUMENTS

The branch state and plan footers above are already loaded. If a PR already
exists for this branch, update its title and body instead of opening a
second one.

Run `just qc` before opening anything — CI runs the same four steps, so a
red `qc` is a red PR and there is no reason to publish one.

If the branch carries no `Plan:` footer, that is fine; build the body from
the commits and keep it short. Do not write an ADR after the fact to fill
out the template — a small change should look like one.
