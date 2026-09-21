---
id: 260920-zqapye
slug: installable-harness
phase: 6
status: Proposed
---

# Phase 6 — Retire `scripts/agents-wire.sh`

## Goal
Move the projection invariants into `pinst harness check` with every finding id
intact, delete the script, and repoint the four callers and three documents that
still name it — so "the harness needs no `scripts/`" is finally true of the
whole harness and not just of the corpus half.

## Why this phase exists
It is last because it is the only phase that can break `just qc` for everyone,
and it should happen after the replacement has been used rather than before.
Until Phase 5 lands, the Rust projector has never installed anything anybody
depended on; after it, retiring the script is a substitution rather than a bet.

Doing it at all is the point of the plan rather than a tidy-up. `260920-wtburh`
recorded leaving the script as RISK-006 and said so explicitly: skill projection
stayed in bash, so the claim that the harness needs no `scripts/` was true of the
corpus checks and not of the recipe. Two implementations of one invariant is the
drift this harness exists to detect, and this one is about itself (ALT-001).

## Steps

- [ ] TASK-031: `src/core/harness/install/check.rs` — port every finding the
      script emits, each id written as a **string literal**, never assembled
      from fragments: the six projection findings (missing, not-a-link, broken,
      orphan, unmanaged, stale) and the five source validations (no-manifest,
      no-name, name-mismatch, no-description, unquoted-value), keeping each
      one's severity and remediation text.
      **Why literals:** `lesson.unenforced` greps `src/` for the id a lesson's
      `**Check:**` line names, and an id built by `format!` from parts would
      compile, pass every test, and break that grep in silence (GUD-002,
      CON-005).
      **Note:** the unquoted-value case now falls out of the strict frontmatter
      parser rather than an `awk` heuristic, so it should get *better*, not just
      ported — the parser already refuses what the heuristic guessed at.
- [ ] TASK-032: `src/core/harness/check.rs` — call the projection checks for
      project scope as part of `pinst harness check`, so one command carries what
      `just harness` previously needed two to say; `src/core/harness/skills.rs` —
      `wired` reads Phase 1's state function instead of its own `.exists()`, so
      the report and the checker cannot disagree about what "wired" means.
- [ ] TASK-033: delete `scripts/agents-wire.sh`; `justfile` — `wire` becomes
      `pinst harness install --scope project --all`, and `harness` drops its
      first line, leaving `pinst harness check`.
- [ ] TASK-034: repoint the remaining callers — `.github/workflows/ci.yml` (the
      harness step), `scripts/distil-guard.sh` (the `WIRE` variable and its two
      call sites), and `.github/workflows/distil.yml` (the `--allowedTools`
      entry naming the script).
      **Why the workflow's allowlist matters:** an unattended run that is denied
      the tool it was told to use fails in a way that looks like the model
      refusing rather than the harness being misconfigured.
- [ ] TASK-035: documentation — `.agents/README.md`'s "How the skills reach a
      runtime", "The checks" and "Adding a skill" sections (step 2 becomes
      `pinst harness install`, and the commands now live in `.agents/commands/`);
      `README.md`'s command table and its `.agents/` row;
      `src/core/harness/root.rs`'s doc comment on `wired_dir`, which currently
      credits a script that will not exist.
      **Careful:** do not write any of the ported finding ids into a doc comment
      or any file under `src/`. A comment containing one satisfies
      `lesson.unenforced` by itself, which is exactly what happened in
      `260920-wtburh` — the lesson resolved against freshly written prose and
      went silent when the real emitter was moved away (RISK-003).
- [ ] TASK-036: prove the enforcement is real, the way that plan proved it: move
      `src/core/harness/install/check.rs` aside, run `pinst harness check`, and
      confirm LESSON-007 now reports as unenforced; restore it and confirm the
      finding clears. Record both outcomes in the run log.
      **Why by experiment and not by reading:** the property is "a grep over the
      tree finds this id in code", and the only way to know a grep finds
      something is to take it away.
- [ ] TASK-037: `.ash/CHANGELOG.log` entry and `just review`, confirming the
      skills report still grades all six skills and that `wired` is true for
      each — the column whose backing function changed in TASK-032.

## Trade-offs & risks
The `--copy` mode of the script survives as `--copy` on install, and `--check`
survives as `pinst harness check`. Nothing the script could do is lost; what is
lost is the ability to run it without a binary, which `260919-zeuuaj` already
made a non-issue — `scripts/install.sh` puts pinst on `PATH` with no toolchain.

CON-005 is the real hazard in this phase and RISK-003 is its trap. Both are
addressed by TASK-036 rather than by care, because care is what failed last
time: the id was preserved, the check was correct, and the lesson still went
unenforced for the length of a review because something else in the tree
happened to contain the string.

`scripts/distil-guard.sh` keeps its own findings and its own shape. This phase
repoints one variable in it; folding the guard into the binary is a different
plan with a different argument.

## Done criteria
- TEST-023: `pinst harness check` reports every finding the script reported, on
  a corpus constructed to trigger each one — an unwired skill, a real file where
  a link belongs, a dangling link, an orphan link, an unmanaged entry, a stale
  copy, and each of the five malformed `SKILL.md` cases.
- TEST-024: `just qc` and `just harness` are green with `scripts/agents-wire.sh`
  deleted, and no file outside `.ash/` still names it.
- TEST-025: TASK-036's experiment — the check module moved away makes
  LESSON-007 report unenforced, and restoring it clears the finding.
- TEST-026: `just review` grades six skills, all `wired`, with the same rates as
  before the switch.
- TEST-027: a fresh clone of this repo, with no install run, is still wired —
  the symlinks under `.claude/` are committed, so `pinst harness check` passes
  on checkout alone.
