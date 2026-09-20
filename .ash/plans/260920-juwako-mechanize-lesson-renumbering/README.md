---
id: 260920-juwako
slug: mechanize-lesson-renumbering
status: Done
created: 2026-09-20
updated: 2026-09-20
areas: [harness, agents, learnings]
summary: Mechanize the remedy for a duplicate LESSON-NNN id, not just its detection.
files_touched: [src/core/harness/renumber.rs, src/core/harness/mod.rs, src/cli/mod.rs, src/cli/commands/harness.rs, src/core/harness/check.rs, .ash/LEARNINGS.md]
---

# Mechanize lesson renumbering

## Context
`LESSON-NNN` in `.ash/LEARNINGS.md` is minted by hand — "highest existing
number plus one" — in a file every parallel branch appends to. It has
collided three times already (LESSON-022's own `Seen in:` line lists two of
them, in `260920-impoxu-daily-distil-action` and
`260920-wtburh-harness-in-the-binary`). PR #17 (branch
`feat/lesson-duplicate-id-check`, this plan's base) added `pinst harness
check` findings — `lesson.duplicate-id` and `lesson.dangling-reference` — so
a collision is now caught by `just qc` instead of drifting unnoticed. But
LESSON-022 names two separate gaps, and that PR only closed one:

> nothing checks that every lesson id is unique or that a reference to one
> still resolves

was fixed. The other half of the same lesson was not:

> The renumbering is manual and silent, which is the part worth fixing

That is what this plan builds: a command that performs the renumbering
itself, safely, and reports what it did.

## Decision
Add `pinst harness renumber-lesson --title "<substring>" [--to LESSON-NNN]
[--against <ref>]`. It locates the one `### LESSON-NNN` heading whose title
contains `--title` (refusing an ambiguous or absent match), renames it to a
free id (computed automatically unless `--to` is given), and rewrites every
literal citation of the old id that the *current branch itself introduced*
— found by diffing against the merge-base with `--against` (default `main`)
and reading only the added lines — leaving anything that already existed
before the branch diverged untouched. When no merge-base can be resolved (no
git repo, unknown ref), it still renames the one heading it was told to, but
degrades to listing every other occurrence in the repo instead of touching
them, rather than guessing. A `--dry-run` (the existing global flag,
already used by `harness index`) previews the same report without writing.

This is preferred over a blind find-and-replace because the id being moved
is not the only thing in the corpus that might already use that number
legitimately (see `ALT-002` below) — the git-diff scope is the one boundary
that is provably safe without reading every hit by hand, which is the exact
manual step this plan removes.

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| Mint lesson ids with a random component, like plan ids (`<yymmdd>-<six letters>`) | Solves the collision at the root, but at the cost of renaming all 31 existing lessons and every place that cites one by number across the repo's prose, skills and doc comments — a large migration to prevent an event that has happened three times in months. The sequential number is also load-bearing for readability: lessons are discussed by number in conversation, and a random id would end that. |
| Blind global literal replace of every `LESSON-<old>` with `LESSON-<new>` | Unsafe: the old number can already have legitimate citations elsewhere in the tree that predate the collision and refer to the *surviving* lesson, not the one being moved. A blind rewrite would silently corrupt those — trading one silent failure (an unrenumbered reference) for a worse one (a reference silently pointed at the wrong lesson). |
| A report-only tool: rename the heading, print every hit, touch nothing else | This is the fallback path this plan keeps for when no merge-base resolves, but as the *only* behavior it stops one step short of what LESSON-022 asks for — printing hits is not fixing them, and the manual step ("grep the whole corpus... and figure out which ones actually refer to it") remains exactly as costly as before. |

## Consequences
The renumbering remedy becomes as mechanical as the detection PR #17 already
made the check: instead of a person grepping the corpus and reading every
hit, `renumber-lesson` does the safe, git-scoped subset of that grep
automatically and prints what remains. It does not eliminate the need to
look at a diff before committing — no tool can guarantee a rewritten
citation was the one intended — but it turns "grep and read the whole
corpus by hand" into "review a `git diff` before committing," the same
review point every other change in this repo already gets.

**Risk:** the git-diff scoping is only as correct as the branch's own
history — a heavily rebased or squashed branch could present a merge-base
that no longer reflects what was actually added, and the tool would then
scope its rewrite to the wrong set of lines. Mitigated by always printing
the full report and never committing anything itself: the change lands in
the working tree, where `git diff` before the commit catches a wrong scope
the same way it would catch a manual mistake.

**Risk:** a repo with no `git` binary on `PATH`, or one that is not a git
worktree at all, cannot resolve a merge-base. Mitigated by the report-only
degrade path — the identified heading is still renamed, and nothing else is
silently guessed at.

**Dependency:** this is the first place `src/` shells out to the system
`git` binary directly (`git merge-base`, `git diff --unified=0`), via
`std::process::Command` — the same pattern `core::probe` and
`core::exec::github_release` already use for other external tools.

**Assumption:** `main` is the right default `--against` ref for this repo —
consistent with `create-pr` and every branch this corpus has been built on
so far.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Build `pinst harness renumber-lesson` | [phase-01.md](phase-01.md) | Done |
| 2 | Point the finding at the remedy, and close LESSON-022's own gap | [phase-02.md](phase-02.md) | Done |

## Affected Files
- `src/core/harness/renumber.rs` (new) — locate-by-title, next-free-id,
  merge-base resolution, diff-hunk parsing, the digit-boundary rewrite, the
  report-only degrade, and unit tests.
- `src/core/harness/mod.rs` — `pub mod renumber;`.
- `src/cli/mod.rs` — `HarnessAction::RenumberLesson` and its args struct.
- `src/cli/commands/harness.rs` — the command handler (human + `--json`
  output, exit 0 / 2).
- `src/core/harness/check.rs` — `lesson.duplicate-id`'s remediation text
  points at the new command.
- `.ash/LEARNINGS.md` — LESSON-022 records that the remedy is mechanized
  too, not only the detection.

## Open Questions
None.
