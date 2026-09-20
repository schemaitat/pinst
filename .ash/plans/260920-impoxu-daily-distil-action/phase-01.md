---
id: 260920-impoxu
slug: daily-distil-action
phase: 1
status: Done
---

# Phase 1 — The gate and the guard, as shell

## Goal
GOAL-001: Write `scripts/distil-guard.sh`, giving the scheduled run both of
the things that keep it cheap and bounded — a model-free answer to "is there
anything to distil?" and a mechanical answer to "is this diff safe to
publish?" — before any workflow exists to call them.

## Why this phase exists
The guards are the load-bearing part of the design, and they are the part that
can be proven at a terminal: `preflight` can be run against today's clean
corpus, and `verify` against a diff constructed by hand. Building them first
means Phase 2's YAML is wiring rather than logic, which is the only way the
rails get tested before they are trusted at 06:00 UTC with nobody watching.
Putting them in `scripts/` also follows the precedent already set by `ash.sh`
and `agents-wire.sh` (CON-002): checks live in shell, run with no build step,
and exit 0 or 3 like everything else in this harness (PAT-001).

## Steps
- [x] TASK-001: `scripts/distil-guard.sh` — create the script with the usual
      header comment, `set -eu`, a `usage`, and a subcommand dispatch over
      `preflight` and `verify`. Why: one file, two verbs, so the workflow
      never grows a second place where policy lives.
- [x] TASK-002: `scripts/distil-guard.sh` — implement `preflight`. Run
      `scripts/ash.sh check --json` and `scripts/ash.sh skills --json`, read
      `.summary.findings` and `.agenda_items`, print one line
      (`distil: N agenda item(s), M finding(s)`), exit 0 when there is work and
      **1 when the corpus is clean**. Why: 1, not 3 — a clean corpus is not a
      finding, and the workflow branches on it rather than failing on it.
      Parse with `python3` (already a dependency of `ash.sh skills`) rather
      than adding `jq`.
- [x] TASK-003: `scripts/distil-guard.sh` — implement `verify`'s path
      allowlist: every path in `git diff --name-only` must match `.ash/**` or
      `.agents/skills/**`. Anything else is finding
      `distil.path-not-allowed.<path>`. Why: SEC-001 is only real if something
      checks it after the fact; the model's own tool allowlist is the first
      line, not the last.
- [x] TASK-004: `scripts/distil-guard.sh` — add the size budget to `verify`:
      `distil.diff-too-large` when changed lines exceed `DISTIL_MAX_LINES`
      (default 500) or files exceed `DISTIL_MAX_FILES` (default 40).
      Why: environment variables so a breach can be re-examined by raising the
      bound in one place, rather than by deleting the check.
- [x] TASK-005: `scripts/distil-guard.sh` — add the append-only rule: compare
      `git show HEAD:.ash/LEARNINGS.md` against the working tree and report
      `distil.lesson-removed.<id>` for any `### LESSON-NNN` heading that no
      longer exists; do the same for `### ISSUE-NNN` headings in every
      `.ash/plans/*/learnings.md`. Why: this is the specific way an
      unattended distiller goes wrong that review is worst at catching — a
      deletion reads as a tidy diff. RISK-001.
- [x] TASK-006: `scripts/distil-guard.sh` — end `verify` by running
      `scripts/agents-wire.sh --check` and `scripts/ash.sh check`, reporting
      `distil.harness-failed` if either exits non-zero. Why: a distillation
      that leaves the corpus failing its own invariants must not become a pull
      request; an unwired skill edit (LESSON-007) is caught here.
- [x] TASK-007: `justfile` — add a `distil-guard` recipe forwarding its
      arguments (`just distil-guard preflight`). Why: the same name locally
      and in CI, so a maintainer debugging a run reproduces it exactly.

## Trade-offs & risks
- RISK-001 is only bounded here, not removed: `verify` can prove nothing was
  deleted and nothing outside the corpus was touched, and can prove nothing at
  all about whether a triage decision was *right*. That judgement is what the
  pull request is for.
- The size budget is a guess (500 lines / 40 files). A first real distillation
  of a busy corpus could legitimately exceed it; the defaults are variables for
  that reason, and the first breach should be read before the bound is raised.
- `preflight` exits 1 for "clean", which is unusual for this repo, where 3
  means findings and 1 usually means the tool itself failed. The comment header
  must say so explicitly, and Phase 2's job condition must not confuse a clean
  corpus with a broken script.

## Done criteria
- TEST-001: `just distil-guard preflight` exits 1 against today's clean corpus
  and prints `0 agenda item(s)`.
- TEST-002: With a hand-made untriaged issue in a scratch plan, `preflight`
  exits 0 and names a non-zero agenda count.
- TEST-003: `verify` exits 3 and names the offending path when the working
  tree touches `src/main.rs`; exits 0 for a diff confined to `.ash/`.
- TEST-004: `verify` exits 3 with `distil.lesson-removed.LESSON-001` when that
  heading is deleted from `.ash/LEARNINGS.md`.
- TEST-005: `just qc` stays green — the script is new and nothing else changed.
