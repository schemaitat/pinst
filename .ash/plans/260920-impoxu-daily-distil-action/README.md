---
id: 260920-impoxu
slug: daily-distil-action
status: In Progress
created: 2026-09-20
updated: 2026-09-20
areas: [ci, harness, agents]
summary: Run the /distil pass on a daily schedule in GitHub Actions, gated by the corpus itself, and land the result as a reviewable pull request instead of a push.
files_touched: [.github/workflows/distil.yml, scripts/distil-guard.sh, justfile, .agents/skills/plan-learnings/SKILL.md, .agents/skills/create-pr/SKILL.md, .agents/README.md, .claude/commands/distil.md]
---

# A daily, unattended distillation pass that opens a pull request

## Context
Distillation is the one lifecycle stage with no owner. `plan-write`,
`plan-implement` and `plan-learnings` each run because someone starts them;
the corpus-wide pass in `plan-learnings` Step 6 — triage every untriaged
issue, reclassify lessons whose enforcement changed, answer every gap group —
runs only when a human types `/distil`. `learnings.untriaged` and the review
agenda make the work *visible*, but visible is not the same as done: the work
still waits on a person noticing a red `qc` and having the time (REQ-001).

The corpus is clean today (`4 plans`, `0 findings`, `nothing to act on`), so
the common case for any scheduled job here is **no work at all** (REQ-003).
That single fact shapes the whole design: a job that wakes a model daily to
discover there is nothing to do would burn budget every morning and train its
reader to skim its output, which is the decay mode this harness exists to
resist.

Two preconditions that blocked earlier CI work have since been fixed, and were
re-verified by query rather than assumed (LESSON-002). The repo is public, and
`repos/schemaitat/pinst/actions/permissions/workflow` now returns
`{"default_workflow_permissions":"write","can_approve_pull_request_reviews":true}`
— so the default `GITHUB_TOKEN` can open a pull request, which is what
ISSUE-001 of `260919-zeuuaj` said it could not. Two that are *not* satisfied:
`repos/schemaitat/pinst/actions/secrets` holds **zero secrets**, so no
Anthropic credential is wired (DEP-002), and the `main-protect` ruleset carries
only `deletion` and `non_fast_forward` — there is no required review, so
nothing mechanical stops a robot's pull request being merged unread (CON-003).

This plan also contradicts a decision this corpus already recorded, and says so
up front rather than quietly: LESSON-013 ("Make the corpus the clock, not the
calendar") and ALT-001 of `260919-vldfei` rejected exactly this — a wall-clock
distillation cadence. That objection is answered, not overruled, in the
Decision below; if the answer does not convince, the honest outcome is to drop
this plan rather than to weaken the lesson.

## Decision
Split the run in two halves with a **gate between them**, and put the gate in
shell, not in the model. A cheap, model-free preflight (`scripts/ash.sh
skills --json`, already emitting `agenda_items`) decides whether there is
anything to distil. Zero agenda items and zero check findings ends the run
there: no model call, no branch, no pull request, a green one-line log
(REQ-003). Only a non-empty agenda starts Claude Code.

That is what answers LESSON-013. The *trigger* stays the corpus — the same
`agenda_items` count that a human's red `qc` shows — and cron is demoted to a
**poller** for it. A quiet week costs four seconds a day and opens nothing; a
busy one is picked up the morning after the plan closes instead of whenever
someone next runs `qc`. What the lesson actually warned about — firing into
silence, living in one person's account — is answered by the gate and by the
workflow being a file in the repo that a fresh clone inherits (GUD-001).

The model runs headless via `anthropics/claude-code-action@v1` with the prompt
`/distil` — the existing slash command, not a copy of it (CON-001). Its write
surface is `.ash/**` and `.agents/skills/**`, and it is given no tool that can
push, tag, or open anything (SEC-001). **Committing, branching and opening the
pull request are ordinary workflow steps**, written in YAML where they can be
reviewed, not actions the model is trusted to perform (ALT-004).

Between the model and the pull request sits `scripts/distil-guard.sh verify`,
which is the real safety rail and the reason the guards are a script rather
than YAML (CON-002): it refuses a diff that touches a path outside the
allowlist, that exceeds a size budget, that deletes an existing `LESSON-NNN`
or an existing `ISSUE-NNN` heading — `LEARNINGS.md` is append-only and a
robot that rewrites history is the failure worth engineering against — or that
leaves `just harness` failing. It runs locally and in CI with the same exit
codes (PAT-001), so the rail is testable without waiting for 06:00 UTC.

Finally, the unattended run **proposes and never writes** a new skill (REQ-004),
which the manual workflow already requires. Unattended, "propose" needs a
destination: proposals are written into the pull request body, under a section
the reviewer cannot miss, and the corresponding `**Gap:** answered — ...` line
is still recorded on the issue so the review stops re-asking (LESSON-014).

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Push the distilled result straight to `main` | Distillation is judgement — promote, merge, or decline — and judgement made by an unattended model is precisely what should be read before it becomes the record the next plan reads. A pull request costs one review and makes every distillation reversible by closing it |
| ALT-002: Inline a copy of the `/distil` prompt into the workflow YAML | A second copy of the instructions drifts from `.claude/commands/distil.md` and from `plan-learnings` Step 6, and the drift is invisible until a run distils by yesterday's rules. The command file is the source of truth; the workflow names it (CON-001) |
| ALT-003: `npm i -g @anthropic-ai/claude-code` and run `claude -p` by hand | Defensible, and kept as the documented fallback if the action misbehaves, but it means owning CLI installation, version pinning and the auth matrix (API key / OAuth / Bedrock / Vertex) that the action already owns |
| ALT-004: Let the model open the pull request itself, via `gh` in its Bash allowlist | Any tool that can open a pull request can push a branch, and the branch policy, the commit message and the PR body then live in the model's head rather than in a file someone can review. Keeping `gh` out of the allowlist is what makes SEC-001 checkable by reading the workflow |
| ALT-005: Use the action's own `branch_prefix` / commit-signing machinery | Same objection one layer down: the commit this repo publishes should be produced by a step this repo wrote. Signing is worth revisiting once the workflow has run unattended for a while |
| ALT-006: One dated branch and one pull request per day | Daily cadence plus per-day branches is a queue of stale, conflicting pull requests nobody closes. One rolling branch, force-updated, means at most one open distillation at a time and the newest one is always the whole answer |
| ALT-007: Keep LESSON-013 as written — no schedule, `learnings.untriaged` in `qc` is the trigger | This is the status quo and the strongest alternative. Rejected because the finding only fires *at* someone already running `qc`; the gate in the Decision keeps the lesson's substance (the corpus decides) while adding the thing the lesson lacks — something that notices when nobody is looking |
| ALT-008: Auto-merge the distillation pull request when CI is green | `main` has no required review (CON-003), so auto-merge would make the robot's judgement the record with no human in the loop at all — the one property this plan spends a pull request to buy |
| ALT-009: Let the run write a proposed skill and mark it draft | "Propose, never write" exists because a harness that accumulates skills nobody invokes is worse than one missing a skill. A draft file under `.agents/skills/` is a written skill with an apology attached, and `agents-wire.sh` would wire it |

## Consequences
- **The harness starts measuring its own robot.** `create-pr`'s evidence
  command counts merged pull requests carrying a `Plan:` footer, and it already
  sits at 7/11. A daily robot pull request would enter that denominator and
  drive a real skill's conformance rate toward zero — a measurement artifact
  that looks exactly like a regression. Phase 3 resolves it deliberately, by
  making the distillation pull request carry `Plan: 260920-impoxu-daily-distil-action`
  (it does implement a plan) and by excluding the robot's author from the
  evidence window.
- RISK-001: An unattended distillation is wrong in a way a human would not be
  — declining an issue that generalizes, or promoting noise into
  `LEARNINGS.md`. Mitigated but not eliminated by review; the pull request is
  the mitigation, and `verify`'s append-only rule bounds the blast radius to
  additions.
- RISK-002: Cost. Bounded by the preflight gate (no agenda, no model call), by
  `--max-turns`, and by a job `timeout-minutes`.
- RISK-003: The action's behaviour with no pull-request or issue context is the
  part of this design least verified from documentation. Phase 2 prints the
  action's full report on the first runs (GUD-002, LESSON-006) rather than
  trusting that automation mode does what the README implies.
- RISK-004: A rolling branch force-updated daily will clobber a reviewer's
  in-progress edits to that branch. Accepted: the branch is robot-owned, and a
  reviewer who wants to edit should branch off it.
- RISK-005: Secrets in logs. The credential reaches only the action's `with:`
  block; no step echoes the environment.
- ASSUMPTION-001: `anthropics/claude-code-action@v1` accepts a bare slash
  command (`/distil`) as its `prompt` and expands the repo's
  `.claude/commands/distil.md`, including its `!`-prefixed context commands.
  **Unverified** — Phase 2's first task is to prove or disprove it, and
  ALT-003 is the fallback if it is false.
- ASSUMPTION-002: A daily cadence is the right one. Nothing here depends on it;
  the cron line is one string, and the gate makes an over-frequent schedule
  cheap rather than noisy.
- DEP-001: `anthropics/claude-code-action@v1`.
- DEP-002: An Anthropic credential in repo secrets — `ANTHROPIC_API_KEY` or
  `CLAUDE_CODE_OAUTH_TOKEN`. **Neither exists today**; this is a human
  prerequisite, and Phase 2 is not verifiable end to end until it is set.
- DEP-003: `extractions/setup-just@v3`, already used by `release.yml`, because
  `/distil` runs `just review`.
- DEP-004: `GITHUB_TOKEN` with `contents: write` and `pull-requests: write`,
  also exported as `GH_TOKEN` for the whole job so `create-pr`'s evidence
  command can reach the API during the review the model reads.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | The gate and the guard, as shell | [phase-01.md](phase-01.md) | Done |
| 2 | The scheduled workflow | [phase-02.md](phase-02.md) | Proposed |
| 3 | Teach the harness about its own robot | [phase-03.md](phase-03.md) | Proposed |

## Affected Files
- FILE-001: `scripts/distil-guard.sh` — new. `preflight` (is there anything to
  distil?) and `verify` (is this diff safe to publish?), with the repo's
  0/3 exit-code contract.
- FILE-002: `justfile` — new `distil-guard` recipe, so both halves of the gate
  are runnable locally by the same name CI uses.
- FILE-003: `.github/workflows/distil.yml` — new. Cron + `workflow_dispatch`
  with a `dry_run` input; preflight job, model job, verify, commit, pull
  request.
- FILE-004: `.claude/commands/distil.md` — a short "unattended mode" section:
  where proposals go, and what the run must not do.
- FILE-005: `.agents/skills/plan-learnings/SKILL.md` — Step 6 gains the
  unattended clause: a proposal's destination is the pull request body.
- FILE-006: `.agents/skills/create-pr/SKILL.md` — `evidence:` excludes the
  robot author from the conformance window.
- FILE-007: `.agents/README.md` — routing table entry, and a line naming the
  workflow as distillation's unattended entry point.

## Open Questions
Seven, all of which change what gets built — they are listed for a decision
before Phase 1 starts, not left for implementation to guess.

1. **ASSUMPTION-001 is unproven** (`/distil` as an action prompt). Resolved by
   the first task of Phase 2; ALT-003 is the fallback.
2. **DEP-002 — which credential.** A `CLAUDE_CODE_OAUTH_TOKEN` bills a Claude
   subscription; `ANTHROPIC_API_KEY` bills the API. Only one needs to exist,
   and only a repo admin can set it.
3. **LESSON-013 / ALT-007.** The Decision answers the objection with the gate.
   If that answer is not convincing, this plan should be dropped rather than
   built — recorded here so the disagreement is visible in the corpus either
   way.
4. **Diff-size breach behaviour.** `verify` fails the job and opens nothing
   (proposed default), versus opening a *draft* pull request marked
   `oversized` so the work is visible rather than discarded.
5. **Where proposals land.** Pull request body (proposed), versus a separate
   GitHub issue per proposal, versus a tracked `.ash/PROPOSALS.md`.
6. **Whether the unattended run may edit an existing skill at all.** The manual
   rule forbids writing a *new* skill and says nothing about editing one.
   Proposed: allowed, since reclassifying a lesson can require it, and
   `just harness` plus review cover it.
7. **CON-003 — no required review on `main`.** A branch ruleset requiring one
   approving review would make "a human read this" mechanical rather than
   customary. Out of scope for this plan; it is a repo setting, not a file.
