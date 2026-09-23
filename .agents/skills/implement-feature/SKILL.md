---
name: implement-feature
description: 'Drive a feature request from a safe isolated worktree through a persisted implementation plan, phase-by-phase delivery, verification, learnings, and a pull request. Use when the user asks to implement, build, or ship a feature end to end.'
produces: 'a feature delivery leaves one persisted plan, a completed implementation run and learnings record, verified changes, and a pull request when publication is authorized'
evidence: 'echo $(for d in .ash/plans/*/; do grep -q ''^status: Done'' "$d/README.md" 2>/dev/null && grep -qh ''"event":"run_start"'' "$d"/logs/*.log 2>/dev/null && grep -qh ''"event":"run_end"'' "$d"/logs/*.log 2>/dev/null && [ -f "$d/learnings.md" ] && echo x; done | grep -c x) $(ls -d .ash/plans/*/ 2>/dev/null | grep -c .)'
---

# Implement Feature

Turn one feature request into one bounded delivery chain. This skill is a
coordinator: it composes the existing lifecycle skills and owns the handoffs
between them. It does not replace their rules, invent a second plan format, or
silently make decisions that belong to the user.

The successful outcome is **fully resolved**: the requested feature is
implemented in an isolated, reviewable branch; its plan is persisted and
complete; every implementation run has a closing outcome and learnings record;
the repository quality gate passes; and a pull request exists if the user
authorized publication. An idle agent, a clean-looking diff, a pushed branch,
or a created PR alone is not resolution.

## Inputs and invariants

Capture these before mutating anything:

- the feature request, acceptance criteria, constraints, and any requested
  agent/runtime;
- the source checkout, current branch, `git status --short`, and
  `git rev-parse HEAD`;
- an explicit base ref if the user supplied one, otherwise the current
  committed `HEAD`;
- whether the user authorized publishing a branch and opening a PR, or only
  asked for local implementation.

The feature is one unit of work and one plan. Do not combine unrelated
requests. Preserve the user's decisions about architecture, scope, target
branch, publication, credentials, trust, and destructive cleanup.

## Step 1 — Establish safe topology

Mutating feature work must happen in a new or already-authorized isolated git
worktree with a unique feature branch. Use the host's worktree/orchestration
facility when one is available; otherwise use the equivalent `git worktree
add` procedure. The new worktree must start from a committed base ref, and its
path and branch must be recorded before planning starts.

Inspect the source checkout first. If it is dirty, or the requested feature
depends on uncommitted parent changes, stop and ask the user to choose a safe
handoff. Never stash, copy, commit, reset, or silently omit those changes.
Never use the parent checkout as a second writer. Never overwrite an existing
worktree or branch, force a branch, or remove a worktree as cleanup.

If the user explicitly supplied an existing feature worktree and branch, verify
that its repository, branch, base, and current changes match the handoff before
using it. A mismatch is a blocker, not permission to repair topology.

## Step 2 — Write and persist the plan

Invoke the existing `plan-write` skill in the selected worktree. Its ADR
format under `.ash/plans/<id>-<slug>/` is the only plan format for this
repository:

1. Read `.ash/LEARNINGS.md` before choosing the approach.
2. If the user supplied a plan id/slug, use that exact plan only after
   verifying it belongs to this feature. Otherwise let `plan-write` mint an
   id using the repository's supported mechanism; never count directories or
   guess among multiple candidates.
3. Persist the README and phase files, regenerate the index, and run the
   plan/harness validation required by `plan-write`.
4. Record the exact plan id and directory as the handoff to implementation.

Do not start implementation until the plan exists and its phases/tasks form
the required linear chain. If planning reveals a material product choice,
ambiguous requirement, or false assumption, pause for the user's decision.

## Step 3 — Execute the plan

Invoke `plan-implement` with the exact persisted plan id. It owns phase order,
task checkboxes, status mirrors, append-only run logs, per-phase commits,
conventional commit footers, changelog entries, and its required
`plan-learnings` handoff. Let it work through all phases without pausing for
routine confirmation between already-approved tasks.

The coordinator must not edit plan files to make progress look complete. If a
phase fails, the plan drifts, or an assumption changes the intended result,
stop the chain and report the concrete condition. Resume only after the user
chooses a plan revision, supplies the missing approval, or explicitly
authorizes a bounded retry.

After `plan-implement` returns, verify:

- the exact plan directory and current status;
- a run log containing matching `run_start` and terminal `run_end`;
- `learnings.md`, including an honest blocked/aborted outcome when applicable;
- the branch and diff contain only this feature's work; and
- every promised phase/task and acceptance criterion is accounted for.

## Step 4 — Verify the delivery

Run the repository's documented quality gate in the feature worktree (for this
repository, `just qc`). Check the feature-specific acceptance criteria and
inspect the final diff and commits. If a check fails:

- fix a regression caused by this feature through the plan's normal
  implementation path, then re-run the relevant phase criteria and `just qc`;
- distinguish a pre-existing failure with reproducible evidence and stop for
  the user's decision; and
- never weaken, skip, suppress, or relabel a check to claim success.

Quality-gate success is necessary but not sufficient: it does not prove the
plan, learnings, branch, or PR handoffs are complete.

## Step 5 — Create the pull request

Only after implementation and verification pass, invoke the existing
`create-pr` skill. It must discover the exact plan from commit footers, build
the PR body from the ADR, preserve Conventional Commit title rules, and
include the plan footer.

Pushing and opening a PR are external, visible mutations. If the user's
request did not clearly authorize publication, pause before `git push` and
`gh pr create` and ask. Never merge, close, force-push, delete branches, or
remove worktrees as part of this skill. A partial plan may be reported or
opened as a draft only when the user explicitly chooses that outcome; it is
not “fully resolved.”

## Blockers, approvals, and terminal outcomes

Use these states in the handoff report:

- **resolved** — isolated branch and path, plan id, completed phases, verified
  acceptance criteria, clean quality gate, learnings path, and PR URL (or a
  user-authorized local-only result with no PR requested);
- **blocked** — an approval, credential, trust decision, dirty-base choice,
  product decision, external permission, or pre-existing failure is required;
  preserve artifacts and ask one focused question;
- **aborted** — the user declined, the requested result is unsafe or
  contradictory, or a bounded retry failed; preserve the worktree and report
  the exact reason.

Never claim resolved while any approval/question, failed required check,
unfinished plan phase, missing learnings/run closure, or unverified artifact
remains. On every non-successful implementation return, ensure
`plan-learnings` has recorded the outcome before reporting it. If the runtime
supports delegated supervision, hand the entire chain to `orchestrate`; the
parent supervisor may inspect and nudge, but must not implement or co-edit the
child worktree.
