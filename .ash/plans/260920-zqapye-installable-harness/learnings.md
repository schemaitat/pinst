---
id: 260920-zqapye
slug: installable-harness
updated: 2026-09-21
areas: [harness, agents, cli, tui]
issue_count: 6
---

# Learnings — the harness becomes something pinst installs (260920-zqapye-installable-harness)

## Source
- Plan: `./README.md`
- Basis: session context
- Logs consulted: `logs/20260920T202919Z-claude-sonnet-5.log` (one run, spanning a session-limit interruption and resume)

## Summary
All six phases landed: `pinst harness install`/`uninstall`/`status`, an
interactive picker, a fifth TUI tab, and `scripts/agents-wire.sh` fully
retired into the binary with its enforcement proven live. The run was
unattended and interrupted once by the account session limit, mid-Phase-1;
it resumed cleanly from the append-only log and git history with no lost
work, which is itself the headline lesson — the discipline `plan-implement`
imposes (commit per phase, log every event, never hand-edit history) is what
made an unattended, interruptible run possible at all. Three real bugs were
caught by hand-testing before they shipped, and two were caught only because
this plan's own testing discipline was applied against its own new code:
a manual filesystem check for wire.missing's literal after the module
was moved aside, and a grep sweep for the class of mistake that check is
supposed to prevent, which found an instance of it in code written minutes
earlier.

## Issues

### ISSUE-001: A session-limit interruption mid-phase cost nothing but time
**What happened:** The run was cut off partway through Phase 1, immediately
before running clippy and tests on a clean build, with six commits' worth of
work still uncommitted. A follow-up message (itself duplicated many times by
the delivery layer) asked to resume "from where the run log left off,"
describing a state — "you have no commits yet" — that was accurate at
interruption time but stale by the time it arrived.
**Root cause:** Nothing in this repo; this is the expected shape of an
unattended run hitting an external resource limit.
**Fix applied:** Re-read the run log's last few lines and `git log`/`git
status` before acting on the resume instruction, found both commits and log
lines existed for phases already done, and continued from the actual state
rather than the stale one the message described.
**Recommendation:** Always verify a resume instruction's factual claims
("no commits yet") against `git log` and the run log before acting on them,
even when — especially when — the instruction sounds confident and was
presumably accurate when it was written. A duplicated or delayed message is
exactly the shape of input that can go stale between being written and being
read. Promoted to LESSON-038.
**Skill:** plan-implement

### ISSUE-002: A command's copy-mode write joined an empty path onto a file target
**What happened:** Installing a command with `--copy` (used by default at
global scope) failed with "is a directory" — `Action::Write`'s target was
built as `target.join(relative_within_asset(...))`, and for a command
`relative_within_asset` returns an empty `PathBuf` by design (a command's
target *is* its one file, unlike a skill's target, which is a directory files
land inside). Joining an empty relative path still appends a path separator,
turning a file-write into "write into a directory of this name."
**Root cause:** One code path was written to serve both skills (many files
under a directory) and commands (one file, no subdirectory) by the same
per-file loop, and the empty-path case for commands was never a real input
during design — only surfaced by actually running a global copy-mode install
end to end, which the unit tests (which asserted plan *shape*, not execution
result) did not do.
**Fix applied:** Split the `LinkStyle::Copy` branch on asset kind: a command
gets one `Action::Write` straight at its target path, a skill keeps the
per-file loop. Added a regression test that builds the plan *and executes
it*, then checks the file actually landed — not just that the action's shape
looked right.
**Recommendation:** For any code path built to serve two similar-but-not-
identical shapes (here: "a target is a directory of files" vs. "a target is
one file"), write at least one test that executes the result end to end for
each shape, not only tests that assert the plan/output structure. A plan
that looks right and a plan that runs right are different claims.
**Skill:** none
**Gap:** answered — this is a code-shape regression, not recurring work a
repository skill should own.
**Distilled:** declined — specific to this plan's particular code shape
(a two-representations-in-one-loop bug), not a pattern this codebase's other
`Plan`-building code repeats today.
**Gap:** answered — this is a representation-specific implementation bug, not
recurring work that a repository skill should own.

### ISSUE-003: A read-only checker picked up the wrong `.agents/` for a test fixture
**What happened:** `install::check::run` was first wired to take a `Source`
from the caller, with the caller (`harness::check::run`) passing
`asset::resolve_source()` — the same ambient, cwd-based resolution `install`/
`status` use. A pre-existing corpus-fixture unit test (`a_healthy_fixture_is_
silent`), which builds a synthetic `.ash/` under a tempdir with no `.agents/`
of its own, started failing: the ambient resolver found *this checkout's*
real `.agents/` (since `cargo test`'s cwd is the repo root) and checked it
against the fixture's unrelated tempdir, producing a dozen bogus
`wire.missing` findings.
**Root cause:** Ambient source resolution is the right default for a command
a human runs with intent ("install from wherever `.agents/` actually is"),
but wrong for a check that runs automatically against a corpus root that has
already been named explicitly — falling back to an unrelated ambient source
answers a question nobody asked, confidently and wrong.
**Fix applied:** `install::check::run` now derives its own source from
`<project_root>/.agents` and returns no findings at all when that directory
does not exist there, rather than accepting a source or falling back to
ambient discovery.
**Recommendation:** When a checker is handed an explicit root it must
validate *against*, resolve every input relative to that root rather than
accepting a separately-resolved value from the caller — a caller-supplied or
ambient value can silently point somewhere else, and the failure mode is a
confident wrong answer, not an error. Promoted to LESSON-039.
**Skill:** none
**Gap:** answered — this is a checker implementation boundary, already
addressed by LESSON-039; no additional workflow skill would prevent it.

### ISSUE-004: The lesson.unenforced trap this module exists about caught this module
**What happened:** While writing `install/check.rs`'s own module-level doc
comment explaining why finding-id prefixes must stay literal, the comment
itself named `wire.missing` in prose — which would have satisfied
`lesson.unenforced`'s grep even if the real emitter were deleted, exactly
because that is what happened once already to a predecessor of this module
(recorded as an issue in `260920-wtburh`'s own learnings). A deliberate grep
sweep for every ported finding id, run before TASK-036's verification
experiment, found this instance and a second one in `state.rs`'s doc comment
(`wire.unmanaged`), both written in this same plan's earlier phases.
**Root cause:** Writing accurate documentation about a naming convention
requires naming an example, and the natural example is the real id — which
is precisely the shape of mistake the convention exists to prevent. The rule
is easy to state and easy to violate while explaining the rule itself.
**Fix applied:** Rephrased both comments to refer to ids "by shape, not by
name," matching the pattern `harness::check`'s own `NOT_SOURCE` comment
already uses for the same reason. Verified by the TASK-036 experiment itself
(moving the real module aside and confirming the finding fires) rather than
by re-reading.
**Recommendation:** After writing any doc comment that explains a
literal-id convention, grep the file(s) just written for every id it names,
before relying on a "the check will catch it" experiment to prove the point
— the experiment proves the *emitter* is real, not that no stray copy
exists elsewhere. This is the second time this exact mistake has been made
in this harness's own code, which is the threshold this file's own
convention treats as "worth promoting." Merged into LESSON-025, which
already covers this exact trap from its first occurrence.
**Skill:** none
**Gap:** answered — this is a narrowly documented harness invariant, already
covered by LESSON-025 rather than a missing workflow skill.

### ISSUE-005: Receipt-update logic was about to be duplicated between the CLI and the TUI
**What happened:** Partway into Phase 5's `run_harness_action` (the TUI's
install/uninstall mutation), the "turn a finished run's reports into an
updated receipt" logic was about to be written a second time, inline, having
already been written inline in `cli::commands::harness`'s `install`/
`uninstall` in Phase 2/3.
**Root cause:** The plan scoped Phase 5 as "reuse the same
build_install_plan/build_uninstall_plan and Runner" but did not call out the
receipt-update step specifically, and that step was originally written
inline in the CLI command function rather than as a reusable unit.
**Fix applied:** Factored it into `core::harness::install::record`
(`record_install`/`record_uninstall`), shared by both the CLI and the TUI;
refactored the CLI's own `install`/`uninstall` to call it too, with a
before/after manual regression check confirming identical CLI behavior.
**Recommendation:** When a phase's plan says "reuse the same X" for a
mutation, check whether everything *downstream* of X (here: recording what
happened) is already factored out too — "the same plan and executor" is not
"the same everything," and the seam usually needs to move one function
earlier than the phase boundary suggests.
**Skill:** plan-write
**Distilled:** declined — a specific instance of "check for shared downstream
steps before splitting work across phases," which is a judgement call each
plan makes about its own shape rather than a general rule a future plan
could act on directly.

### ISSUE-006: TASK-010 could not be verified live; the plan's own documented fallback was followed
**What happened:** Phase 2's TASK-010 called for confirming, in a live
Claude Code session, whether `~/.claude/commands/` is actually read at
global scope. No live session was available (unattended overnight run).
**Root cause:** Not an implementation defect — a verification step that
genuinely requires an environment this run did not have.
**Fix applied:** Followed the fallback the plan itself specified for exactly
this case: install both skills and commands at global scope by default
(the full feature) rather than pre-emptively narrowing to skills-only,
reasoning that an unused file is a cheaper wrong guess than a silently
withheld feature. Recorded as an `issue` event in the run log at the moment
of the decision, and left as an open question in the plan's own README.
**Recommendation:** None beyond what the plan already did: writing the
fallback and its reasoning into the plan *before* implementation started
(rather than deciding under pressure at the point of being blocked) is what
let this proceed without stopping or guessing. Worth continuing as a pattern
for any TASK that depends on an environment an unattended run cannot
guarantee.
**Skill:** none
**Gap:** answered — this is an environment-specific verification limitation,
not recurring work a repository skill should own.
**Distilled:** declined — already the exact practice LESSON-003 argues for
(splitting a phase's Done criteria into what's provable locally and what
needs a merge/environment); this is that pattern applied, not a new one.
**Gap:** answered — this was an environment-limited verification choice with
an explicit fallback in the plan, not recurring work for a new skill.
