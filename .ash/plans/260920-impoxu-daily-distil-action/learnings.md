---
id: 260920-impoxu
slug: daily-distil-action
updated: 2026-09-20
areas: [ci, harness, agents]
issue_count: 5
---

# Learnings — a daily, unattended distillation pass (260920-impoxu-daily-distil-action)

## Source
- Plan: `./README.md`
- Basis: session context, cross-checked against this run's log
- Logs consulted: `./logs/20260920T114850Z-claude-opus-5.log`

## Summary
Phase 1 landed complete: `scripts/distil-guard.sh` and its `just distil-guard`
recipe, with all five Done criteria verified against a throwaway clone rather
than against the live tree. Phase 2 is blocked before its first task, because
settling ASSUMPTION-001 needs a live workflow run and the repo still holds zero
Actions secrets (DEP-002) — a block the plan predicted rather than discovered.
The headline lesson is not about CI at all: two of the four defects in this
phase were `set -euo pipefail` turning an ordinary `grep` idiom into a silent
abort and a phantom finding, in a repo whose entire enforcement layer is bash
written exactly that way.

## Issues

### ISSUE-001: `set -e` killed the guard on a plan that had no issues
**What happened:** `verify` exited 1 with no output partway through the
append-only check. The trace ended at `before=` while reading
`260918-lmmebj`'s `learnings.md`.
**Root cause:** `before="$(heading_ids_at_base "$file")"` is a plain
assignment, so — unlike `local x=$(...)` — it propagates the command's exit
status, and `set -e` acts on it. The two legacy plans genuinely record no
issues, so `grep` found nothing and exited 1. The guard treated "this plan had
a clean run" as a fatal error, and said nothing at all about it.
**Fix applied:** `|| true` on both assignments, with a comment naming the
legacy plans so the next reader knows the empty case is real data rather than
an oversight.
**Recommendation:** in a `set -e` script, any assignment whose right-hand side
is a command substitution over `grep`, `find` or `git` needs `|| true` unless
an empty result really is fatal. The tell is a script that exits with no
output: `set -e` aborts silently, so the absence of a message is the message.
**Skill:** none
**Gap:** answered — not skill-shaped. No instruction prevents a `set -e`
assignment swallowing grep's exit status; LESSON-015 is the durable form, and a
linter (`shellcheck SC2312`-class) would beat prose if this recurs.

### ISSUE-002: `printf | grep -q` reported a lesson that was present as removed
**What happened:** `verify` raised `distil.lesson-removed.LESSON-013` against
an untouched `LEARNINGS.md`, and the id it named changed between runs.
**Root cause:** `printf '%s\n' "$after" | grep -qx "$id"` — `grep -q` exits on
its first match, `printf` then dies of `SIGPIPE`, and `set -o pipefail`
promotes that 141 to the pipeline's status, so the `&& continue` never ran and
the loop fell through to `finding`. Which id it hit depended on where in the
buffer `grep` happened to stop, which is why it looked random.
**Fix applied:** `grep -qx "$id" <<< "$after"` — a here-string instead of a
pipeline, so there is nothing to receive a `SIGPIPE`.
**Recommendation:** never put `grep -q` (or `head`, or any early-exiting
reader) on the right of a pipe in a `pipefail` script. Feed it a here-string or
drop the `-q`. This one is worth catching by eye, because the symptom — a
check that fails intermittently on correct input — reads as flakiness in the
data rather than a bug in the checker, and that is the most expensive kind of
wrong answer a guard can give.
**Skill:** none
**Gap:** answered — not skill-shaped, same reason as ISSUE-001. The two
together are, however, the strongest argument yet for a skill covering how this
repo writes its shell checkers; raised as a proposal, not written.

### ISSUE-003: the plan's `git diff` could not see the files a distillation actually creates
**What happened:** TASK-003 specified the path allowlist over `git diff
--name-only`. That set excludes untracked files, and the artifacts of a
distillation — a new `learnings.md`, a new plan directory — are created, not
modified.
**Root cause:** the plan reached for the obvious command for "what changed"
without asking what an unattended pass would actually produce. A run that wrote
a brand-new file outside the corpus would have passed the allowlist unseen,
which is precisely the hole SEC-001 exists to close.
**Fix applied:** `changed_paths()` unions `git diff --name-only` with `git
ls-files --others --exclude-standard`; a test proved it now catches an
untracked `.github/workflows/sneaky.yml`.
**Recommendation:** when a guard is written against a diff, decide first
whether the thing being guarded against arrives as a modification or as a new
file, and write the test for the second case before the first. The version that
only sees modifications passes every test built from edits.
**Skill:** none
**Gap:** answered — not skill-shaped. This is a property of `git diff`,
learned by writing the wrong test first; LESSON-016 carries it.

### ISSUE-004: the gate wakes the model on findings that are not distillation work
**What happened:** flipping `phase-01`'s status to `In Progress` made
`ash.sh check` report `index.stale`, and `preflight` answered `work=true` on
it — it would have woken a model to regenerate an index.
**Root cause:** the plan gates on `agenda_items` *or* any check finding.
Untriaged issues already reach `agenda_items`, so the findings half adds little
beyond false wakeups from mechanical findings (`index.stale`,
`phase.status-mirror`) that `just qc` already catches and that no distillation
pass should be spent on.
**Fix applied:** unresolved — implemented as the plan specifies. Narrowing the
gate would change what the plan delivers, so it was raised rather than decided
mid-implementation.
**Recommendation:** gate on the specific findings that represent the work
(`learnings.untriaged`, `lesson.unenforced`) rather than on the finding count,
or gate on `agenda_items` alone. Decide it in Phase 2, where the cost of a
false wakeup is a real model call.
**Skill:** none
**Distilled:** declined — an open design decision inside this plan's own Phase
2, not a lesson: which findings the gate reads is still being chosen.
**Gap:** answered — not skill-shaped. A design decision deferred to the
phase that pays its cost, which is judgement no instruction encodes.

### ISSUE-005: Phase 2 stops at its first task, on a precondition only a human can satisfy
**What happened:** TASK-008 requires a live workflow run to settle
ASSUMPTION-001, and `repos/schemaitat/pinst/actions/secrets` reports zero
secrets, so there is no credential for the action to authenticate with.
**Root cause:** not a defect — the plan named DEP-002 as a human prerequisite
and split Phase 2's Done criteria into what is provable in the tree and what
is not (LESSON-003). The block arrived as a documented stop rather than as a
surprise at the end of a long run.
**Fix applied:** unresolved by design — the user has chosen
`CLAUDE_CODE_OAUTH_TOKEN` and will set it; the run ended `blocked` at
phase 2 / TASK-008, with the resume point recorded in the log.
**Recommendation:** keep putting the task that settles an unverified
assumption first in its phase. The cost of this block is one phase boundary;
had TASK-008 sat at the end, it would have been the cost of a whole workflow
written against a premise that might not hold.
**Skill:** none
**Distilled:** declined — LESSON-002 and LESSON-003 already say this; the run
is those two lessons working as intended, which is not itself a new lesson.
**Gap:** answered — not skill-shaped. A GitHub secret only a repo admin
can set is the archetype of a domain surprise, and LESSON-002 already tells a
plan to check for it up front, which this one did.

