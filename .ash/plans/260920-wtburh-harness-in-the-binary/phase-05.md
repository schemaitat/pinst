---
id: 260920-wtburh
slug: harness-in-the-binary
phase: 5
status: Proposed
---

# Phase 5 — Cutover: retire the script, rewire the callers

## Goal
Make `pinst harness` the only implementation: delete `scripts/ash.sh`, point
the justfile, the skills, the slash commands and the READMEs at the binary,
and regenerate the index the new generator now owns.

## Why this phase exists
Every earlier phase deliberately left `scripts/ash.sh` in place so the port
could be diffed against it — TEST-013 and TEST-019 are comparisons that only
exist while both do. Cutting over earlier would have destroyed the oracle
before it was used; cutting over in the same phase as any single action would
have left the corpus checked by two half-implementations. This phase is the
moment the comparison is finished and the second copy becomes the drift
ALT-003 was rejected for.

## Steps
- [ ] TASK-032: delete the comparison tests from phases 3 and 4 (TASK-022 and
      the `ash.sh` half of TEST-019) — Why: after TASK-033 there is nothing
      for them to compare against, and a test whose oracle has been deleted is
      a fixture pretending to be a check.
- [ ] TASK-033: delete `scripts/ash.sh` (FILE-017).
- [ ] TASK-034: `justfile` (FILE-016) — `index` becomes `cargo run --quiet --
      harness index`; `harness` becomes `scripts/agents-wire.sh --check` plus
      `cargo run --quiet -- harness check`; `review` runs `harness check` then
      `harness skills`, keeping the worst exit code. Why: RISK-002 — note on
      the `index` recipe that it now needs a build, and keep the `qc` ordering
      (`fmt-check lint test harness`) so the build is already warm by the time
      `harness` runs.
- [ ] TASK-035: `justfile` — the `review` recipe keeps running both halves
      regardless of the first's exit code and returns the worst. Why: a
      failing `check` must not hide the report; this is existing behaviour and
      the easiest thing to lose while rewriting the recipe body.
- [ ] TASK-036: `.agents/skills/plan-write/SKILL.md` (FILE-019) — Step 2's
      `scripts/ash.sh new-id` becomes `pinst harness new-id`, and Step 7's
      `scripts/ash.sh index` / `check` references follow. Why: this file's own
      `evidence:` one-liner does not touch `ash.sh`, so the frontmatter needs
      no change — only the prose does.
- [ ] TASK-037: `.agents/skills/plan-learnings/SKILL.md` (FILE-020) — the
      `check` and `skills` references, plus a sentence recording that a
      lesson's `**Check:**` id is now searched for under `src/` as well as
      `scripts/` (TASK-020). Why: the skill is where an author learns what
      makes a `mechanized` claim valid, and the valid set just widened.
- [ ] TASK-038: `.claude/commands/plan.md` (FILE-021) and
      `.claude/commands/implement.md` (FILE-022) — the pre-loaded `new-id` and
      `check` calls. Why: these run on every planning and implementation
      session; a stale command here fails at the start of the next plan rather
      than at review.
- [ ] TASK-039: `.agents/README.md` (FILE-018) — every `scripts/ash.sh …`
      invocation in "The checks", "Grading the skills", "Invocation counts"
      and "The review cadence", and the `new-id` line under the plan-id
      explanation. Why: this is the file an agent reads to learn the harness;
      it names the script eleven times.
- [ ] TASK-040: `README.md` (FILE-024) — add `harness` to the command table
      alongside `docs`, `doctor` and the rest, described as the corpus checks
      rather than as machine state. Why: the user-facing promise this plan
      makes is "a downloaded binary offers the same corpus checks", and the
      README is where that promise is discoverable.
- [ ] TASK-041: run `cargo run -- harness index` to regenerate `.ash/INDEX.md`
      (FILE-023), whose attribution comment now names the new generator, then
      `just qc`. Why: phase 2 deliberately deferred this so the tree never
      carried an index naming a command that did not fully exist; the
      `index.stale` check will report it if this step is skipped.

## Trade-offs & risks
RISK-006 survives this phase by design: `just harness` still shells to
`scripts/agents-wire.sh --check`, so "the harness needs no `scripts/`" is true
of the corpus checks and not of the recipe. Say so plainly in `.agents/README.md`
rather than leaving the reader to discover it — a half-finished claim read as
finished is worse than an honest limitation. Porting wire as `pinst harness
wire` is a follow-up plan.

RISK-002 is now permanent: there is no bash fallback. If that turns out to
hurt — a machine with no Rust toolchain that needs to check a corpus — the
answer is to install the pinst binary there, which is the property this plan
was written to deliver, not to restore the script.

This is also the phase where ASSUMPTION-002 becomes expensive to reverse.
Restoring `scripts/ash.sh` from git is trivial; restoring it *and keeping it
correct* is the cost, and it is the cost ALT-003 was rejected over.

## Done criteria
- **TEST-025**: `scripts/ash.sh` does not exist, and `grep -rn "ash\.sh"
  --include='*.md' --include=justfile .` returns hits only under
  `.ash/plans/` — the historical record, which is allowed to name a file that
  is gone.
- **TEST-026**: `just qc` is green, and `just harness`, `just index` and
  `just review` all behave as before the cutover.
- **TEST-027**: `.ash/INDEX.md` is regenerated, its attribution comment names
  `pinst harness index`, and `pinst harness check` reports no `index.stale`.
- **TEST-028**: on a machine with only the release binary — `just dist`, then
  run the extracted binary from a clone of this repo with `cargo` unavailable
  on `PATH` — `pinst harness check` and `pinst harness skills` both produce
  the same verdict as the source build. Verifiable in the working tree by
  clearing `PATH` down to the binary's directory plus `git` and `sh`.
- **TEST-029**: `pinst harness check` run from a directory containing only a
  `.ash/` tree copied out of this repo — no `manifest.toml`, no
  `.agents/` — still validates the corpus, proving discovery is not tied to a
  pinst checkout (REQ-005).
- **TEST-030**: `pinst harness new-id` is what `/plan` calls, confirmed by
  running the slash command once and seeing an id minted without a
  `scripts/ash.sh: not found`.
