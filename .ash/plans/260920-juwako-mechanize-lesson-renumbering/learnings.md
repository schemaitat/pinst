---
id: 260920-juwako
slug: mechanize-lesson-renumbering
updated: 2026-09-20
areas: [harness, agents, learnings]
issue_count: 5
---

# Learnings — mechanize the remedy for a colliding LESSON-NNN (260920-juwako-mechanize-lesson-renumbering)

## Source
- Plan: `./README.md`
- Basis: session context, cross-checked against this run's log
- Logs consulted: `./logs/20260920T185251Z-claude-opus-5.log`

## Summary
Both phases landed in one run, with no task failing and no Done criterion
missed: `pinst harness renumber-lesson` ships, `lesson.duplicate-id`
remediates by naming it, and LESSON-022 records that its remedy is
mechanized and not only its detection. Two of the plan's own
implementation details turned out to be wrong when written against the
real code, and both were wrong in the same way — each named a specific
git command or a specific existing helper to reuse, and each was chosen
before anyone had checked what that command or helper was actually scoped
to. Neither changed what the plan delivers, which is why implementation
continued rather than going back to `plan-write`; the headline lesson is
that a plan naming a *mechanism* buys precision at the cost of being the
part most likely to be wrong.

A fifth issue was found afterwards, on review, and is the most serious of
the five: the remediation phase 2 added named the colliding *id* where the
command expects a *title*, so following it failed every time — and the test
written for it passed, because it matched the message's wording instead of
running what the message said. `just qc` was green throughout.

## Issues

### ISSUE-001: The plan's diff command would have rewritten the wrong lines in a dirty tree
**What happened:** TASK-002 specified `git diff --unified=0 <merge-base>
HEAD` as the source of the line numbers to rewrite. Those numbers index
into the *new file* as of `HEAD`, but the tool writes to the working
tree. Any uncommitted edit above a citation shifts it, and the rewrite
would have landed on a neighbouring line — silently, since a wrong line
still writes successfully.
**Root cause:** the diff was specified as a way of answering "what did
this branch add", which `merge-base..HEAD` does answer correctly. What it
does not answer is "where is that line right now", and those are the same
question only while the tree is clean. The tool exists to be run at the
moment someone is mid-way through resolving a collision, which is
precisely when it is not.
**Fix applied:** diffed `merge-base` against the working tree
(`git diff --unified=0 --relative <merge-base>`, no second commit), so the
line numbers describe the files being written. A second guard was added on
top: a line is only rewritten when it still carries the old id, so a
number that has gone stale for any other reason drops out of the report
rather than corrupting a line.
**Recommendation:** when a diff is being used to *locate an edit* rather
than to describe history, diff against the tree the edit will land in.
Promoted as LESSON-032.
**Skill:** plan-write

### ISSUE-002: The helper the plan said to reuse was scoped to the opposite purpose
**What happened:** TASK-003 said the degrade path should reuse
`check.rs`'s `contains_literal`/`NOT_SOURCE` walk. `NOT_SOURCE` skips
`.ash`, and the walk skips every `.md`, `.txt` and `.log` file. Applied
here, it would have reported almost no citations at all — lesson
citations are overwhelmingly markdown inside `.ash`.
**Root cause:** those exclusions exist so that a lesson cannot be
satisfied by the prose describing it (LESSON-025); they encode
`lesson.unenforced`'s intent, not a general "files worth searching". The
plan saw a walk over the repo looking for a literal and recognised the
shape, not the intent — and the intent is the part that does not travel.
**Fix applied:** a small walk local to `renumber.rs` with its own skip
list (`.git`, `target`, `node_modules`), documented against
`check::NOT_SOURCE` so the difference reads as deliberate rather than as
an oversight. It also returns `file:line` rather than a bool, which
`contains_literal` could not have done anyway.
**Recommendation:** before reusing a filter constant, read why its
entries are there. Promoted as LESSON-033.
**Skill:** plan-write

### ISSUE-003: The task that made the module compile came four tasks after the tasks that tested it
**What happened:** TASK-001 through TASK-003 each called for unit tests,
but `pub mod renumber;` was scheduled in TASK-004. Until that line exists
the file is not part of the crate, so none of those tests can run — the
first three tasks were unverifiable as written.
**Root cause:** the plan grouped every edit by *file*, putting all the
`mod.rs`/`cli` wiring in one "this is only wiring" task. That reads well
and is wrong for exactly one line of it: the module declaration is not
wiring, it is what makes the code exist.
**Fix applied:** added the one-line declaration during TASK-001 and left
the rest of the wiring in TASK-004, logged as an `issue` at the time.
**Recommendation:** this is a one-line reordering with no consequence
beyond itself, and the existing LESSON-016 ("add an API in the phase that
consumes it") already points at the same instinct from the other
direction. Not worth a lesson of its own.
**Distilled:** declined — a one-line sequencing nit inside a single
phase, already adjacent to LESSON-016.
**Skill:** plan-write

### ISSUE-004: The run log's own helper silently transposed two fields
**What happened:** two `phase_done` lines were written with the event's
message in the `task` field and its `detail` JSON in `message`, because
the shell helper used for logging takes four positional arguments and was
called with three.
**Root cause:** a positional-argument wrapper shifts silently when an
argument is omitted; nothing about the call site said which field was
which.
**Fix applied:** the two lines were corrected in place and an `issue`
event appended saying so, rather than appending duplicate `phase_done`
events that would then double-count.
**Recommendation:** none that generalizes — the log format is specified
by the `plan-implement` skill and writing it with a named-argument helper
(or inline) avoids this entirely.
**Distilled:** declined — a slip in a throwaway session helper, not in
anything the repo keeps.
**Gap:** answered — not skill-shaped. No instruction prevents a positional
shell argument from shifting; the log format is already specified, and the
fix is to write the line rather than to write a wrapper.
**Skill:** none

### ISSUE-005: The remediation named the one string the command cannot match
**What happened:** `lesson.duplicate-id` remediated with `pinst harness
renumber-lesson --title "LESSON-001"`. `renumber::locate` matches against a
heading's *title* — everything after the id — which can never contain the id
itself, so the suggested command failed with "no lesson heading's title
contains 'lesson-001'" every single time it was followed. Caught by a human
on review, not by `just qc`.
**Root cause:** two causes, and the second is the one worth keeping. The
finding had only the `Lesson` model to hand, which carried an `id` and no
title, so the id is what it printed — the data available shaped the message
rather than the consumer's requirement. And the test written alongside it
asserted that the remediation string *contained* `pinst harness
renumber-lesson` and `LESSON-001`; both were true of a command that did not
work, so the test was green standing exactly where the bug was.
**Fix applied:** `corpus::Lesson` gained a `title`, parsed by a single
`split_lesson_heading` shared with `renumber::headings` so the two cannot
drift about where an id ends. The finding now names every colliding heading
by title and offers one command per heading, one finding per colliding id
rather than one per extra heading. `locate` prefers an exact title match over
a substring one, so a complete pasted title cannot come back ambiguous just
because it is a prefix of a longer lesson. The test now parses the `--title`
values back out of the remediation and runs `locate` with each — verified to
fail against the old remediation with the reported error — and a corpus-wide
test asserts every real lesson is reachable by its own title.
**Recommendation:** execute the command a message suggests. Promoted as
LESSON-034.
**Gap:** answered — not skill-shaped. No instruction would have caught a
remediation naming the wrong field; the test that runs the suggested command
is the enforcement, and it now exists.
**Skill:** none
