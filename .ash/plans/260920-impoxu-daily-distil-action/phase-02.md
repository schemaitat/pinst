---
id: 260920-impoxu
slug: daily-distil-action
phase: 2
status: Proposed
---

# Phase 2 — The scheduled workflow

## Goal
GOAL-002: Add `.github/workflows/distil.yml` — cron plus manual dispatch,
gated by Phase 1's `preflight`, running `/distil` headless under a constrained
tool allowlist, and turning whatever it changed into a pull request against a
single rolling branch (REQ-001, REQ-002, REQ-005).

## Why this phase exists
It depends on Phase 1 for both of its decision points: whether to start the
model at all, and whether to publish what it produced. Written before the
guards exist, this workflow would either embed that logic in YAML — where it
cannot be run locally — or ship without it. It is also the phase that first
spends money and first talks to GitHub, so everything mechanical that can be
settled beforehand has been.

## Steps
- [ ] TASK-008: `.github/workflows/distil.yml` — settle ASSUMPTION-001 first:
      a `workflow_dispatch`-only skeleton with `prompt: /distil`,
      `display_report: true` and `show_full_output: true`, run once by hand.
      Why: GUD-002/LESSON-006 — read the third-party action's actual report on
      the first run instead of inferring its behaviour from a README. If the
      slash command does not expand, switch to ALT-003 (`npm i -g
      @anthropic-ai/claude-code` and `claude -p "/distil"`) before building the
      rest on a false premise.
- [ ] TASK-009: `.github/workflows/distil.yml` — add the triggers and
      top-level blocks: `schedule: - cron: "17 6 * * *"`, `workflow_dispatch`
      with a boolean `dry_run` input, `permissions: {contents: write,
      pull-requests: write}`, and `concurrency: {group: distil,
      cancel-in-progress: false}`. Why: an off-the-hour minute avoids the
      congested top of the hour; `cancel-in-progress: false` because a
      half-cancelled distillation leaves a branch mid-update.
- [ ] TASK-010: `.github/workflows/distil.yml` — the `gate` job: checkout with
      `fetch-depth: 0`, `extractions/setup-just@v3`, then `just distil-guard
      preflight`, publishing `work=true|false` as a job output. Why:
      `fetch-depth: 0` is not optional — `conventional-commits`' evidence
      command reads `git log`, and a depth-1 clone would grade a 100% skill on
      one commit. DEP-003.
- [ ] TASK-011: `.github/workflows/distil.yml` — fail the gate job closed when
      no Anthropic credential is present (SEC-002): a step that exits non-zero
      with a one-line remediation if both `ANTHROPIC_API_KEY` and
      `CLAUDE_CODE_OAUTH_TOKEN` are empty. Why: an unset secret otherwise
      surfaces as a model step that fails obscurely, or worse as a job that
      looks skipped — which is indistinguishable from "nothing to do" (DEP-002).
- [ ] TASK-012: `.github/workflows/distil.yml` — the `distil` job, `needs:
      gate`, `if: needs.gate.outputs.work == 'true'`, `timeout-minutes: 30`,
      with `GH_TOKEN: ${{ github.token }}` exported for the whole job. Why: the
      `create-pr` evidence command shells out to `gh pr list`, and without a
      token the review the model reads carries a spurious
      `skill.evidence-failed.create-pr` (DEP-004). RISK-002 is bounded here by
      the timeout.
- [ ] TASK-013: `.github/workflows/distil.yml` — the model step:
      `anthropics/claude-code-action@v1` with `prompt: /distil`, the
      credential, and `claude_args` carrying `--max-turns`, an explicit model,
      and an `--allowedTools` list of `Read,Edit,Write,Glob,Grep` plus only the
      Bash commands the pass needs (`just review`, `scripts/ash.sh *`,
      `just harness`, `git diff`, `git status`). Why: no `gh`, no `git
      commit`, no `git push` anywhere in that list — ALT-004 and SEC-001 are
      enforced by what is absent.
- [ ] TASK-014: `.github/workflows/distil.yml` — after the model, run `just
      distil-guard verify`. Why: the point of Phase 1. A failure here ends the
      run with no branch and no pull request, and the job log carries the
      finding that explains why.
- [ ] TASK-015: `.github/workflows/distil.yml` — the no-op exit: if `git
      status --porcelain` is empty, log `nothing to distil` and end the job
      successfully without a branch. Why: the gate answers "was there an
      agenda", this answers "did the pass actually change anything" — a pass
      that triages everything into declines can legitimately produce an empty
      diff, and an empty pull request is noise (REQ-003).
- [ ] TASK-016: `.github/workflows/distil.yml` — commit to the rolling branch
      `harness/distil`: configure the bot identity, commit with a Conventional
      Commits subject (`chore(harness): distil the corpus (<date>)`) carrying
      the `Plan: 260920-impoxu-daily-distil-action` footer, and force-push.
      Why: ALT-006 — one rolling branch, so at most one distillation is open;
      the footer is what keeps the commit inside this repo's own conventions.
- [ ] TASK-017: `.github/workflows/distil.yml` — open or update the pull
      request with `gh pr create ... || gh pr edit`, body assembled from the
      model's report and an explicit `## Proposed skills — not written`
      section, plus a `needs-human-judgement` label. Why: REQ-004 — an
      unattended proposal needs a destination a reviewer cannot miss, and the
      idempotent create-or-edit is what makes a daily rerun converge rather
      than duplicate. Never `--auto-merge` (ALT-008).
- [ ] TASK-018: `.github/workflows/distil.yml` — honour `dry_run`: run
      everything through `verify`, upload the patch with
      `actions/upload-artifact`, and skip TASK-016/TASK-017. Why: REQ-005 — a
      way to see exactly what tomorrow's run would do without it doing
      anything, which is also how the first real distillation should be
      inspected.

## Trade-offs & risks
- RISK-003 concentrates here. If TASK-008 shows that automation mode does
  something other than what this phase assumes, the phase changes shape — which
  is why it is the first task and not the last.
- RISK-004 (a force-pushed rolling branch clobbering reviewer edits) is
  accepted rather than mitigated: the alternative is a stale-branch queue
  (ALT-006), which is worse.
- RISK-005: the credential is referenced once, in the action's `with:`. No step
  echoes the environment, and `show_full_output` is intended to be turned back
  off once TASK-008 has answered its question.
- The cron time (06:17 UTC) is arbitrary and worth revisiting only if it turns
  out to collide with something; ASSUMPTION-002 says nothing depends on it.

## Done criteria
Split per LESSON-003, because half of this phase cannot be proven in the
working tree.

**Provable locally / in the tree:**
- TEST-006: `actionlint` (or `gh workflow view`) accepts `distil.yml`, and the
  `--allowedTools` list contains no `gh`, `git commit`, `git push` or `git tag`.
- TEST-007: `just distil-guard preflight` and `verify` are invoked by the
  workflow under the same names a maintainer would type.
- TEST-008: `just qc` is green.

**Only confirmable after the workflow runs (and after DEP-002 is set by an
admin):**
- TEST-009: A `workflow_dispatch` run with `dry_run: true` on a clean corpus
  ends at the gate, calls no model, and opens nothing.
- TEST-010: A `workflow_dispatch` run against a corpus with a seeded untriaged
  issue produces a patch artifact whose diff is confined to `.ash/`.
- TEST-011: A live run opens exactly one pull request on `harness/distil`, and
  a second run the same day updates it instead of opening a second.
