---
id: 260919-vldfei
slug: self-improving-agent-harness
phase: 3
status: Proposed
---

# Phase 3 — Skill contracts and the artifact audit

## Goal
GOAL-003: Make every skill declare what it leaves behind, and add
`scripts/ash.sh skills` — a graded report of, per skill: whether it is wired,
whether it declares a contract, how well its artifacts conform over a window
and over all history, and how many recorded issues name it.

## Why this phase exists
It depends on Phase 2 alone, which established both the fields and the
precedent that the corpus can fail a build over them. This phase is
deliberately *not* part of `check`: rates and tallies are information, not
invariants, and `report()` returns 3 for a finding of any severity, so putting
a rate in `check` would mean `qc` fails whenever a number is interesting
(CON-003). Separating them also keeps the blast radius small — a wrong
conformance measure misleads a reviewer, it does not block the repo.

## Steps
- [ ] TASK-011: `.agents/skills/*/SKILL.md` — add two frontmatter keys to all
      six skills: `produces:`, a one-line English claim about the artifact the
      skill leaves in the repo, and `evidence:`, the shell command that
      measures conformance and prints `<conforming> <total>`. `pinst` declares
      `produces: none` with a `reference` marker — it is a contract for driving
      a CLI, not a producer of artifacts, and that exemption must be explicit
      rather than an absence.
      Why: the skill is the only place that knows what it is for. Centralising
      the measures in the script would put the definition of "did create-pr
      work" somewhere `create-pr` cannot see, and the two would drift — which
      is the failure this whole plan is about.
- [ ] TASK-012: `scripts/ash.sh` — add the `skills` subcommand: enumerate
      `.agents/skills/*/`, read the two keys, run each `evidence:` command with
      a timeout, and report per skill. Findings for structural facts only —
      `skill.no-contract.<name>` when the keys are missing,
      `skill.evidence-failed.<name>` when the command errors — never for a low
      rate, which is reported as a number.
      Why: PAT-001 and CON-002. Reusing `finding()`/`report()` gives the JSON
      envelope and exit codes for free (ALT-007), and keeps one definition of
      what a finding looks like.
- [ ] TASK-013: `scripts/ash.sh` — implement the six `evidence:` commands as
      the skills' own one-liners, each windowed and all-time:
      `conventional-commits` → subjects matching the type/scope grammar in
      `git log` (100% of 30 today, the baseline); `create-pr` → merged PR
      bodies carrying a `Plan:` footer, via `gh`, degrading to `unmeasured`
      when `gh` is absent or offline (RISK-003); `plan-write` → plan
      directories that pass `ash.sh check` and carry every ADR section;
      `plan-implement` → plans whose `logs/` hold a log with a matching
      `run_start`/`run_end` pair; `plan-learnings` → `Done` plans with a
      `learnings.md`, times the triage rate from Phase 2; `pinst` → exempt.
      Why: each of these measures the skill's actual deliverable, not a proxy
      for it (RISK-005). A conforming commit *is* what `conventional-commits`
      is for; there is nothing to game that is not also the goal.
- [ ] TASK-014: `scripts/ash.sh` — report both a windowed rate (last N units,
      default 20) and the all-time rate, labelled distinctly.
      Why: RISK-002. Thirty conformant commits make the thirty-first
      non-conformant one invisible in an all-time rate; a window is what makes
      a regression show up while it is still one commit old.
- [ ] TASK-015: `scripts/ash.sh` — join the per-skill `**Skill:**` tallies from
      Phase 1 onto the report, so each row carries how many recorded issues
      implicate it.
      Why: this is the "does it fail" half of the question. A skill with high
      conformance and three issues naming it is producing conforming artifacts
      by a procedure that keeps going wrong — the most valuable row in the
      table, and invisible from conformance alone.
- [ ] TASK-016: `justfile` — add `review: ` running `ash.sh check` then
      `ash.sh skills`, and leave `harness` and `qc` untouched.
      Why: one command for the review pass, and an explicit guarantee that the
      graded report never gates a commit.
- [ ] TASK-017: `.agents/README.md` — document the contract block, what each
      column of the report means, and the rule that a low rate is a
      conversation rather than a failure.
      Why: a number with no stated meaning gets optimised or ignored, and both
      are worse than reading it.

## Trade-offs & risks
- RISK-005 (Goodhart) is structural here. The defence is the choice of
  measures: each is the skill's deliverable, so the only way to improve the
  number is to do the thing. Any future measure that fails that test does not
  belong in `evidence:`.
- RISK-003 accepted: `create-pr` is the one measure needing the network. It
  reports `unmeasured` rather than `0`, because a zero that means "offline"
  is worse than a gap that says so.
- Accepted: running a command out of a skill's frontmatter is executing
  repo-controlled content. It is no more privileged than `justfile` or the
  scripts already in `qc`, but it is worth stating that `evidence:` is code
  and gets reviewed as code.
- Deferred: no historical trend — the report describes now, not last month.
  Trends need storage, and ALT-005 rejected a second store. Git history is the
  store; if trends are wanted later, they are a `git log` over this file.

## Done criteria
- TEST-008: `ash.sh skills` on today's corpus reports six rows, exits 0, and
  reproduces the baselines confirmed while planning — `conventional-commits` at
  30/30 all-time, `plan-implement` with exactly one plan carrying a complete
  run log.
- TEST-009: removing the two keys from one `SKILL.md` produces
  `skill.no-contract.<name>` and exit 3; restoring them clears it.
- TEST-010: a deliberately failing `evidence:` command produces
  `skill.evidence-failed.<name>` rather than a crash or a silent zero.
- TEST-011: `ash.sh skills --json` is valid JSON carrying every row, and
  `just qc` remains green and unchanged in content — the graded report is not
  in it.
