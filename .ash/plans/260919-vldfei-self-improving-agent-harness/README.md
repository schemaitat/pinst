---
id: 260919-vldfei
slug: self-improving-agent-harness
status: Proposed
created: 2026-09-19
updated: 2026-09-19
areas: [agents, harness, learnings]
summary: Make the harness measure itself — lessons with a lifecycle, distillation enforced by a finding, and a skill audit that reports what fires, what conforms, what fails, and what is missing.
files_touched: [.agents/README.md, .agents/skills/plan-learnings/SKILL.md, .agents/skills/conventional-commits/SKILL.md, .agents/skills/create-pr/SKILL.md, .agents/skills/plan-write/SKILL.md, .agents/skills/plan-implement/SKILL.md, .agents/skills/pinst/SKILL.md, scripts/ash.sh, justfile, .ash/LEARNINGS.md, .ash/plans/260918-lmmebj-self-contained-toolchain-cli/learnings.md, .ash/plans/260918-pzdyxp-dotfiles-manager-tui/learnings.md, .ash/plans/260919-zeuuaj-release-please-binary-artifacts/learnings.md, .claude/commands/distil.md]
---

# A self-improving agent harness

## Context
The harness writes prolifically and reads almost nothing back. `.ash/` holds
three plans, 21 recorded issues and ten distilled lessons; `.agents/skills/`
holds six skills. Nothing measures whether any of it works. Distillation only
happens when `plan-learnings` runs at a plan's close, so a lesson learned on a
Tuesday reaches `LEARNINGS.md` only if someone remembers to finish that plan
(REQ-001) — and a lesson, once written, stays prose forever with no notion of
having been *acted on* (REQ-002). Nothing knows whether a skill ever fires,
whether the work it produces conforms to what it promises, whether it has been
the direct cause of recorded failures, or whether some recurring job is being
done ad hoc because no skill covers it (REQ-003, REQ-004, REQ-005).

This is not hypothetical. Today's drift — a plan advertising work that had
shipped a day earlier — was caught by a human asking, not by the harness, and
the fix was a check that compares the corpus against a record written after the
fact (LESSON-009). The same move generalizes: the harness already emits enough
evidence to grade itself, and three sources were confirmed by direct query
while writing this plan.

- **git history**: 30 of 30 commits parse as Conventional Commits and 14 carry
  a `Plan:` footer. `conventional-commits` has a measurable 100% conformance
  rate, and it is measurable because the commit *is* the deliverable.
- **`logs/*.log`**: `plan-implement` already writes JSONL with typed events —
  `run_start`, `phase_start`, `task_done`, `task_failed`, `issue`,
  `phase_done`, `commit`, `run_end`. The one existing run recorded 11 `issue`
  events.
- **session transcripts** under `~/.claude/projects/*/*.jsonl`: 14 files for
  this repo, with skill invocations recorded as `Skill` tool calls. They also
  demonstrate their own unreliability, which is the most useful thing they
  proved: slash-command invocations are a *different* record shape
  (`<command-name>`), and `plan-implement` shows **zero** invocations of either
  kind despite having written a complete run log. Counting invocations alone
  would have graded the most-used lifecycle skill as dead.

## Decision
Grade skills on the **artifacts they leave in the repo**, not on whether they
were invoked. Each skill declares in its frontmatter what it is supposed to
produce and the command that measures conformance; `scripts/ash.sh skills`
evaluates those declarations and reports per skill. Invocation counts from
session transcripts stay an opt-in enrichment behind a flag, never a dependency
(CON-001) — they corroborate, they do not decide.

Make the distillation cadence **event-driven and enforced**, not scheduled. A
lesson's life is `prose → mechanized → retired`, recorded on the lesson itself
along with the finding id that now enforces it, which gives "distilled" an end
state instead of an ever-growing pile (REQ-002, LESSON-008). An issue in a
plan's `learnings.md` that no lesson references and that has not been
explicitly declined becomes a `ash.sh check` finding, so a closed plan starts
failing `just qc` until its issues are triaged (REQ-001). The clock is the
corpus, not a calendar.

Keep the two halves apart. `ash.sh check` carries only hard invariants and must
stay silent on a clean corpus, because `report()` returns 3 for a finding of
*any* severity and a check that always speaks is a check that gets disabled
(CON-003). The graded report — conformance rates, invocation counts, candidate
missing skills — is `ash.sh skills`, which `just review` runs and `just qc`
does not.

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Distil on a wall-clock schedule (cron, a scheduled cloud agent, `/loop`) | The trigger is a plan closing, not a Tuesday. A weekly job fires into silence on a quiet week and misses four plans on a busy one, and it lives in one person's account rather than in the repo, so a fresh clone does not inherit it. A finding fires exactly when there is something to distil, and everyone who runs `qc` sees it |
| ALT-002: Measure skills from session transcripts alone | Vendor-specific (`~/.claude/…`, outside the repo, violating CON-001), prunable, undocumented in shape, and provably incomplete: slash commands record differently and `plan-implement` shows zero invocations despite a run log it clearly wrote |
| ALT-003: Score each skill with an LLM rubric per session | Unreproducible and unauditable — two runs disagree, and nobody can diff a score. It costs a model call per measurement and is exactly the "invariant that lives only in prose" LESSON-008 warns about. Judgement belongs in the review agenda, not in the measurement |
| ALT-004: Build the audit into `pinst` itself | pinst's domain is the machine's toolchain, not this repo's prose corpus, and the harness checks are deliberately shell so they run with no build step on a machine that has not compiled anything |
| ALT-005: A metrics store (SQLite, a JSON sidecar) accumulating run statistics | The corpus already is the store. A second store is a second thing to drift from reality, and the first thing this plan exists to fix is drift |
| ALT-006: Auto-promote lessons to `LEARNINGS.md` at plan close | Promotion is the judgement step — deciding a lesson generalizes is the whole value. Automating it is how `LEARNINGS.md` becomes long enough that nobody reads it, which the `plan-learnings` skill already warns about |
| ALT-007: A new `scripts/harness-audit.sh` instead of extending `ash.sh` | Would duplicate `finding()`, `report()`, the JSON envelope and the exit-code contract. The evidence it reads — plans, issues, lessons — is corpus data, so it belongs to the script that already owns the corpus |

## Consequences
- **DEP-001** nothing new. The checks stay bash + awk + git, as
  `agents-wire.sh` and `ash.sh` are today (CON-004, ASSUMPTION-001).
  **DEP-002** `python3`, only inside the opt-in transcript path, where JSONL
  parsing is not worth writing in awk. **DEP-003** `gh`, only for the
  `create-pr` conformance measure, which degrades to "unmeasured" when it is
  absent or the network is (RISK-003).
- **SEC-001**: the transcript path reads arbitrary conversation text from
  outside the repo — other projects, pasted secrets, unrelated work. It may
  emit **derived counts only**, never a quoted line, a path, or an argument
  string, and it writes nothing into `.ash/`.
- **CON-002**: new subcommands join the existing contract — JSON envelope with
  `schema_version`, findings with a stable `id` and a `remediation` string,
  exit `0` clean / `2` usage / `3` found things to act on (PAT-001).
- **PAT-002**: generated content is regenerated, never hand-edited. Nothing in
  this plan adds a file a human is expected to keep in sync by hand.
- **GUD-001**: every new check compares the corpus against evidence written
  *after* the fact — the changelog, the run logs, git — because that is the
  only kind that can contradict it (LESSON-009).
- **RISK-001**: a check that fires on every closed plan becomes noise and gets
  switched off. Mitigated by scoping it to `ISSUE-NNN` entries rather than
  prose, and by making "declined" a one-line answer that silences it
  permanently.
- **RISK-002**: a conformance rate over all history is dominated by history and
  stops responding to change. Both the windowed rate (last N) and the all-time
  rate get reported.
- **RISK-004**: the transcript schema is undocumented and can change under us.
  The path is opt-in, its failures are non-fatal, and nothing in `qc` depends
  on it.
- **RISK-005**: Goodhart — a skill optimising the number that measures it.
  Mitigated by measuring artifacts that *are* the skill's deliverable (a
  conforming commit, a plan that passes `check`), not proxies for it.
- **ASSUMPTION-002**: the cadence belongs in the repo rather than in a personal
  scheduler. Recorded because the requester was unavailable when this was
  written; ALT-001 remains one recipe away if they prefer a clock.
- **ASSUMPTION-003**: skill attribution on issues is written by
  `plan-learnings` going forward and backfilled once by hand — nothing infers
  it automatically.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Give lessons a lifecycle and issues an owner | [phase-01.md](phase-01.md) | Proposed |
| 2 | Make undistilled learnings a finding | [phase-02.md](phase-02.md) | Proposed |
| 3 | Skill contracts and the artifact audit | [phase-03.md](phase-03.md) | Proposed |
| 4 | Corroborate with invocation evidence | [phase-04.md](phase-04.md) | Proposed |
| 5 | Name the gaps and close the loop | [phase-05.md](phase-05.md) | Proposed |

## Affected Files
- **FILE-001** `.ash/LEARNINGS.md` — every lesson gains `Status:` and, when
  mechanized, `Check:`; the ten existing lessons are backfilled.
- **FILE-002** `.ash/plans/*/learnings.md` (three files) — every `ISSUE-NNN`
  gains a `**Skill:**` line naming the skill it implicates, or `none`.
- **FILE-003** `.agents/skills/plan-learnings/SKILL.md` — documents both new
  fields, the triage step, and the corpus-wide distillation mode.
- **FILE-004** `.agents/skills/*/SKILL.md` (all six) — frontmatter gains
  `produces:` and `evidence:`; `pinst` declares `produces: none` as a
  reference skill.
- **FILE-005** `scripts/ash.sh` — new findings `learnings.untriaged` and
  `lesson.unenforced` in `check`; new `skills` subcommand; `--transcripts`.
- **FILE-006** `justfile` — a `review` recipe; `harness` unchanged so `qc`
  stays free of the graded report.
- **FILE-007** `.agents/README.md` — the lesson lifecycle, the skill contract
  block, the review cadence, and what `skills` reports.
- **FILE-008** `.claude/commands/distil.md` — entry point for the corpus-wide
  distillation pass.

## Open Questions
- **ASSUMPTION-002** (cadence in-repo vs. a personal scheduler) and
  **ASSUMPTION-003** (attribution written by hand, not inferred) are both
  decided by default rather than by the requester, and both are cheap to
  reverse. Everything else the plan assumed was checked by query before this
  file was written.
