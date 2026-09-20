---
id: 260920-juwako
slug: mechanize-lesson-renumbering
phase: 1
status: Proposed
---

# Phase 1 — Build `pinst harness renumber-lesson`

## Goal
Ship the command itself: given an unambiguous title substring, rename the
matching `### LESSON-NNN` heading to a free id and safely rewrite the
citations the current branch itself introduced, with a dry-run preview and
a report-only degrade when the safe scope can't be determined. This is the
whole mechanism; phase 2 only points existing prose at it.

## Why this phase exists
Everything here is one cohesive unit — a single new command with one job —
and none of it is useful half-built: a heading-rename with no safe rewrite
is a worse tool than the manual grep it replaces (it would look done while
leaving the actual toil in place), so it is not worth its own phase
boundary. Phase 2 is separated because it edits *other* files (`check.rs`'s
remediation text, `LEARNINGS.md`'s own prose) that only make sense to touch
once the command they refer to actually exists.

## Steps
- [ ] TASK-001: `src/core/harness/renumber.rs` (new) — read
      `.ash/LEARNINGS.md`, find every `### LESSON-NNN: <title>` heading,
      and locate the one whose title contains a given substring
      (case-insensitive). Zero or more than one match is an error naming
      the candidates. Add a function to compute the next free id (max
      existing numeric id + 1, matching its zero-padded width) that also
      rejects a caller-supplied `--to` already in use. Unit tests
      (TEST-001, TEST-002) cover both, using the plain in-memory text
      fixtures the existing `corpus.rs`/`check.rs` tests already build with
      `tempfile`.
      Why: this is the part of the tool that needs no git at all, so it is
      the cheapest slice to get right first, and TASK-002 builds directly on
      the `(old_id, new_id)` pair it produces.
- [ ] TASK-002: `src/core/harness/renumber.rs` — add merge-base resolution
      (`git merge-base HEAD <against>` via `std::process::Command`, DEP-001)
      and unified-diff hunk parsing (`git diff --unified=0 <merge-base>
      HEAD`) that extracts, per changed file, the *added* lines and their
      line numbers in the new file. Filter those lines for the literal
      `old_id` token using digit-boundary matching (PAT-002, the same rule
      `lesson_ids` in `corpus.rs` already uses, so `LESSON-1` can never eat
      part of `LESSON-19`), and rewrite the heading plus every such line in
      place. Unit test (TEST-003) builds a real tempdir git repo — a commit
      on `main`, a divergent branch that adds a colliding lesson and a
      citation of it in the same diff, plus an older, unrelated citation of
      the same number predating the branch — and asserts the new citation
      moves while the old one does not.
      Why: depends on TASK-001's id pair; this is the part of the tool that
      makes the rewrite *safe*, which is the whole reason this plan exists
      rather than a five-line blind `sed`.
- [ ] TASK-003: `src/core/harness/renumber.rs` — when `git merge-base`
      fails (bad `--against`, no git repo, no `git` on `PATH`), degrade to
      renaming only the located heading and returning every other literal
      occurrence of `old_id` in the repo as a report instead of rewriting
      them (reusing `check.rs`'s `contains_literal`/`NOT_SOURCE` walk, made
      `pub(crate)` if it is not already visible from this module). Add a
      `dry_run: bool` parameter threaded through both the scoped and
      degraded paths that computes and returns the same report without
      writing anything. Unit tests (TEST-004, TEST-005).
      Why: depends on TASK-002's scoped path existing, so the degrade can be
      written as "the same report type, minus the write" rather than a
      second thing to keep in sync.
- [ ] TASK-004: `src/core/harness/mod.rs` — add `pub mod renumber;`.
      `src/cli/mod.rs` — add `HarnessAction::RenumberLesson
      (HarnessRenumberLessonArgs)` with fields `title: String`, `to:
      Option<String>`, `against: Option<String>` (default `"main"` applied
      in the handler, not in `clap`, so `--json` output can show what was
      actually used). `src/cli/commands/harness.rs` — add the handler:
      calls `renumber::run(...)`, prints the report in human mode (heading
      renamed, each rewritten citation as `file:line`, and — in degrade
      mode — each *unrewritten* hit it found instead), returns exit 0 on
      success and 2 on a usage error (ambiguous/absent title, `--to`
      already in use), following the existing `mint`/`index` handlers'
      shape in the same file.
      Why: depends on TASK-001–003 existing as a callable library function;
      this is only wiring.
- [ ] TASK-005: Run `just qc` and fix anything red (TEST-006).
      Why: depends on TASK-004 — nothing compiles as a whole until the CLI
      wiring lands.

## Trade-offs & risks
RISK-001 (git-diff scoping is only as good as the branch's real history)
and RISK-002 (no usable `git` at all) are both plan-level, carried in the
README's `## Consequences` rather than repeated here — this phase is where
they are actually implemented (the degrade path in TASK-003 *is* the
mitigation for RISK-002).

## Done criteria
- [ ] TEST-001: locating a heading by title substring succeeds when exactly
      one matches, and fails naming the candidates when zero or more than
      one do.
- [ ] TEST-002: the next free id is one past the current maximum, at the
      same zero-padded width; a caller-supplied `--to` that already names an
      existing heading is rejected.
- [ ] TEST-003: in a fixture git repo, a citation added by the current
      branch's own diff is rewritten; a citation of the same old number that
      predates the branch's divergence from `main` is left untouched.
- [ ] TEST-004: an unresolvable `--against` (or no git repo) still renames
      the located heading, writes nothing else, and reports every other
      literal occurrence of the old id found in the repo.
- [ ] TEST-005: with dry-run set, the command returns the same report as a
      real run and the working tree is unchanged.
- [ ] TEST-006: `just qc` is green (fmt, clippy, tests, harness).
