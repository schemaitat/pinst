---
allowed-tools: Bash(git add:*), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git commit:*), Read
argument-hint: [optional steer — a type, scope, or what the change is about]
description: Commit the current changes with a Conventional Commits message
---

## Context

- Current git status: !`git status`
- Staged and unstaged changes: !`git diff HEAD`
- Current branch: !`git branch --show-current`
- Recent commits (for type/scope vocabulary): !`git log --oneline -15`
- Scopes already used in this repo: !`git log --format=%s -100 | grep -oE '\(([a-zA-Z0-9_.,/ -]+)\)' | sort -u`

## Your task

Create a single git commit for the changes above, following the
Conventional Commits convention documented in
`.agents/skills/conventional-commits/SKILL.md` — read that file and apply
it rather than working from memory, since it carries this repo's specific
type/scope vocabulary.

User's steer for this commit (may be empty): $ARGUMENTS

If that steer names a type, scope, or describes the change, treat it as a
strong signal — but check it against the actual diff. If the diff
contradicts the steer, say so and propose the message you believe is
correct instead of committing something you know misrepresents the change.

If nothing is staged yet, stage the specific files that belong to this
change (never `git add -A` or `git add .`), and re-check `git status`
afterward to confirm nothing unrelated came along.

If the staged changes plainly cover more than one unrelated concern, stop
and propose splitting them into separate commits rather than forcing one
message to cover both.
