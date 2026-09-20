---
id: 260920-impoxu
slug: daily-distil-action
updated: 2026-09-20
areas: [ci, harness, agents]
issue_count: 13
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

A second run wrote the workflow itself (Phase 2, TASK-009 to TASK-018), which
lints clean but is knowingly unverified: its first task was to settle
ASSUMPTION-001 with a live run, and there is still no credential to run one
with. The phase stays `In Progress` with that one task unticked rather than
being called Done on the strength of a workflow nobody has executed.

Phase 3 then landed complete, and the most valuable thing in it was a side
effect: reading `create-pr`'s own instructions in order to change its
measurement caught a release-breaking bug in the workflow written an hour
earlier.

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
assignment swallowing grep's exit status; LESSON-019 is the durable form, and a
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
learned by writing the wrong test first; LESSON-020 carries it.

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

### ISSUE-006: the task that was meant to de-risk the phase is the one that could not run
**What happened:** TASK-008 exists to settle ASSUMPTION-001 — whether the
action expands `/distil` as a prompt — before the rest of Phase 2 is built on
it. It needs a live workflow run, and `actions/secrets` still reports zero, so
TASK-009 to TASK-018 were written ahead of it on the user's direction.
**Root cause:** the task's dependency is a person, not a predecessor. The plan
correctly put it first and correctly named DEP-002, but a linear chain has no
way to express "this link is held by someone outside the room".
**Fix applied:** unresolved — the workflow ships unproven. TASK-008 is left
unticked, the phase stays `In Progress`, and the mitigations that make the
first real run cheap (`dry_run`, `display_report`, `show_full_output`) are all
in the file.
**Recommendation:** when a phase's de-risking task is gated on an external
actor, say so in the task text and give the phase an explicit fallback order,
so building ahead is a decision the plan anticipated rather than one taken
under pressure at the boundary. The alternative — stopping the phase dead — is
right only when the assumption's falsity would invalidate more than it costs to
rewrite.
**Skill:** none
**Distilled:** merged into LESSON-003 — work complete in the tree but blocked
on someone else's action is exactly the split that lesson exists for.
**Gap:** answered — not skill-shaped. A GitHub secret only an admin can set is
a domain surprise; LESSON-002 and LESSON-003 already carry the response.

### ISSUE-007: an unquoted tool allowlist would have become two broken entries
**What happened:** `claude_args` was first written with
`--allowedTools Read,...,Bash(just review),...` unquoted. `claude_args` is
split shell-style, so `Bash(just review)` would have arrived as `Bash(just`
and `review)`.
**Root cause:** a value containing spaces was written into a blob that is
parsed as a command line, not as YAML strings.
**Fix applied:** the whole allowlist is one double-quoted argument, with a
comment in the workflow saying why the quoting is load-bearing.
**Recommendation:** treat any multi-line `*_args` input to a third-party action
as a command line and quote accordingly. The failure mode is not an error — it
is a permission entry that silently does not match, which on an allowlist means
the tool is denied and the run stalls rather than anything failing loudly.
**Skill:** none
**Distilled:** declined — specific to this action's `claude_args`, and recorded
where it is actionable: a comment on the line itself.
**Gap:** answered — not skill-shaped. Third-party input parsing is a property
of someone else's action, not of how this repo works.

### ISSUE-008: the dry-run patch had the same blind spot the path allowlist did
**What happened:** the `dry_run` artifact was built from `git diff`, which
omits untracked files — most of what a distillation produces.
**Root cause:** the identical mistake as ISSUE-003, made again two hours later
in the same plan, in the second place that reads a diff.
**Fix applied:** `git add -N .` before `git diff`, so created files appear in
the patch.
**Recommendation:** grep for every `git diff` in a change once one of them has
been caught by this. The lesson was already written down when this instance was
introduced, which is the whole argument for mechanizing it rather than
remembering it.
**Skill:** none
**Distilled:** merged into LESSON-020 — a second occurrence, inside the plan
that produced the lesson.
**Gap:** answered — not skill-shaped. A `git diff` default is a domain fact;
LESSON-020 carries it and a check would carry it better.

### ISSUE-009: automating a step inherits every constraint the manual one had
**What happened:** the workflow's pull request title was
`[260920-impoxu] Distil the corpus (<date>)`. This repo is squash-only
(`gh repo view` confirms `mergeCommitAllowed:false`,
`rebaseMergeAllowed:false`), so a squashed title becomes the commit subject on
`main`, which release-please parses. That title produces no version bump and no
changelog entry, silently.
**Root cause:** the bracket prefix comes from `plan-write`, which mandates it
for pull request titles. `create-pr` overrides it and says exactly why — and
the workflow was written without reading `create-pr`, because writing YAML did
not feel like opening a pull request. The constraint belongs to the artifact,
not to who produces it.
**Fix applied:** the title is `chore(harness): distil the corpus (<date>)`,
the plan id moved to the body, and the reasoning sits in a comment on the line
so the next person to edit it sees the trap.
**Recommendation:** before automating a step a skill already owns, read that
skill and satisfy it as written. The tell is the shape of the failure: this one
would not have failed anything — no red job, no error — it would just have
quietly stopped releasing.
**Skill:** create-pr
**Gap:** answered — a skill does own this work; `create-pr` already carried the
rule, and the miss was not reading it.

### ISSUE-010: the conformance metric was already measuring the wrong population
**What happened:** narrowing `create-pr`'s evidence to exclude the robot
revealed that release-please's three merged pull requests had been in the
denominator all along. The rate was reported as 7/11, 63%; the eight human
pull requests are 7/8, 87%.
**Root cause:** the evidence command counted every merged pull request, on the
assumption that every one came from this skill. Two robots open pull requests
here, and neither is this skill being used.
**Fix applied:** both halves of the evidence filter `author.login !=
"app/github-actions"`, with a section in the skill explaining why the exclusion
is of an author rather than of an inconvenient result.
**Recommendation:** when a skill is graded by an artifact that something else
also produces, name the population in the evidence command on the day it is
written. A rate that silently counts a bot's output reads as a regression in
the skill, and the natural response — pressure on whoever is "not following"
it — is aimed at nobody.
**Skill:** none
**Distilled:** merged into LESSON-012 — grading by artifact is right, and this
is the missing half of it: the artifact has to be attributable.
**Gap:** answered — not skill-shaped. Which authors share an artifact is a
property of this repo's automation, learned by adding to it.

### ISSUE-011: `plan-implement` has no event for being unblocked inside one session
**What happened:** this run logged `run_end` with `status: blocked` when
Phase 2 stalled, and then continued — the user unblocked it in the same
session — so Phase 3's events sit after a `run_end` in an append-only log.
**Root cause:** the event vocabulary assumes a run maps to a session. Blocked
and then resumed twenty minutes later by a conversation is neither a new run
nor a continuation the format can express.
**Fix applied:** an `issue` event explains the out-of-order `run_end` in place,
rather than rewriting the log to look tidy.
**Recommendation:** either log `run_end` only when the session ends, or add a
`run_resume` event. The first is simpler and loses nothing — `blocked` is
already recoverable from the last `issue`.
**Skill:** plan-implement
**Distilled:** declined — a small format gap in one skill, fixable in that
skill rather than worth a lesson for every future plan to read.
**Gap:** answered — `plan-implement` owns this; its event table is one line
short, which is a change to that skill, not a missing skill.

### ISSUE-012: `LESSON-NNN` collides across branches, which plan ids are designed not to
**What happened:** merging `main` before opening the pull request produced a
conflict in `.ash/LEARNINGS.md` where both branches had minted `LESSON-015`,
`LESSON-016` and `LESSON-017` for entirely different lessons.
**Root cause:** a plan id is `<yymmdd>-<six random letters>` specifically so
that two sessions in parallel worktrees cannot compute the same value —
`.agents/README.md` explains this at length. `LESSON-NNN` is a plain counter in
an append-only file that every branch appends to, so it reproduces the exact
collision the plan scheme exists to prevent, one directory away.
**Fix applied:** `main` landed first and kept 015–018; this branch's three were
renumbered to 019–021, and the references to them in its own `learnings.md` and
in one `CHANGELOG.log` entry were updated by hand.
**Recommendation:** renumbering works but is manual and silent — nothing would
have caught a missed reference. Either mint lesson ids the way plan ids are
minted, or add a check that every `LESSON-NNN` is unique and every reference
resolves. The second is cheap and would have caught this at `just qc`.
**Skill:** plan-learnings
**Gap:** answered — `plan-learnings` owns lesson numbering; this is a change to
that skill and a check beside it, not a missing skill.

### ISSUE-013: a conflict-marker regex written for the wrong merge style
**What happened:** the resolution script matched
`<<<<<<< / ======= / >>>>>>>` and silently swept a `||||||| 0cc43ca` line into
the "mine" side of both `.ash/LEARNINGS.md` and `.ash/CHANGELOG.log`.
**Root cause:** this repo's merges produce diff3-style markers, which carry a
fourth marker and a base section the three-marker form does not have.
**Fix applied:** the stray lines were found by grepping for all four markers
and deleted; the base content deduplicated away on its own because this branch
only ever appended.
**Recommendation:** grep for `^|||||||` as well as the other three before
declaring a conflict resolved, and grep the whole directory rather than the
files you think you touched.
**Skill:** none
**Distilled:** declined — a one-off scripting slip, caught within a minute by
the check that should follow any scripted resolution.
**Gap:** answered — not skill-shaped; no instruction prevents a regex that is
one alternative short.
