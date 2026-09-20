---
id: 260920-wtburh
slug: harness-in-the-binary
phase: 3
status: Proposed
---

# Phase 3 — `pinst harness check` — every corpus invariant

## Goal
Port all 26 corpus findings from `scripts/ash.sh check` into Rust with their
ids, severities and remediation strings unchanged, so `pinst harness check`
and `scripts/ash.sh check` agree finding-for-finding on the real corpus.

## Why this phase exists
This is the half of the harness that `just qc` gates on, so it is the only
part whose behaviour must be provably identical before anything is retired —
and the only honest way to prove that is to run both against the same corpus
while both still exist. Phase 5 cannot delete the script until this phase has
compared them. It comes after `index` because `index.stale` is one of the 26,
and before `skills` because `skills` reuses this phase's issue and lesson
readers without adding invariants of its own.

## Steps
- [ ] TASK-014: `src/core/harness/check.rs` (FILE-007) — the plan-directory
      and README findings: `corpus.missing`, `plan.malformed-dir.<name>`,
      `plan.duplicate-id.<id>`, `plan.readme-missing.<name>`,
      `plan.frontmatter-missing.<name>`, `plan.frontmatter-key.<name>.<key>`,
      `plan.id-mismatch.<name>`, `plan.slug-mismatch.<name>`,
      `plan.status-invalid.<name>`, `plan.status-section.<rel>`. Why: ids are
      match keys for agents — they carry the corpus-relative path, never the
      absolute one, so a finding id does not change with the checkout
      location.
- [ ] TASK-015: `src/core/harness/check.rs` — add `plan.frontmatter-unparsed.
      <name>` for a document phase 1's strict parser rejects, carrying the
      line number in its message. Why: the one finding this port adds, and it
      exists because a refusing parser needs somewhere to put the refusal
      (RISK-003). It is not a new *invariant* — it is the reporting channel
      for a class of malformed document the awk reader silently tolerated.
- [ ] TASK-016: `src/core/harness/check.rs` — the phase-chain findings:
      `phase.malformed-name.<rel>`, `phase.chain-gap.<name>`,
      `phase.frontmatter-missing.<rel>`, `phase.frontmatter-key.<rel>.<key>`,
      `phase.id-mismatch.<rel>`, `phase.number-mismatch.<rel>`,
      `phase.status-invalid.<rel>`, `phase.status-mirror.<name>.<n>`,
      `plan.done-with-open-phase.<name>`, `plan.phases-all-done.<name>`.
      Why: the mirror check parses the README's `## Phases` markdown table by
      column; keep that a small dedicated function rather than a regex, since
      it is the one place the checker reads prose rather than frontmatter.
- [ ] TASK-017: `src/core/harness/check.rs` — the staleness findings against
      `.ash/CHANGELOG.log`: `plan.unlogged.<name>`,
      `phase.logged-missing.<name>.<n>`, `phase.logged-not-done.<name>.<n>`,
      and `plan.learnings-missing.<name>` (warning when the plan is `Done`,
      info when a run log carries `"event":"run_end"`). Why: GUD-001 — these
      are the checks that compare the corpus against a record written after
      the fact, and `phase.logged-not-done` is LESSON-009's named enforcement,
      so its id must survive the port letter-for-letter (RISK-005).
- [ ] TASK-018: `src/core/harness/corpus.rs` — the distillation readers:
      `issue_states` (`ISSUE-NNN` plus `**Distilled:** declined`),
      `issue_skills` (plus `**Skill:**` and `**Gap:** answered`), `seen_pairs`
      and `lesson_states` over `.ash/LEARNINGS.md`. Why: `seen_pairs` buffers a
      whole wrapped `**Seen in:**` line and emits the cross-product of the plan
      ids and issue ids it finds — deliberately over-generous, because a
      spurious pair can only *silence* a finding and a check that cries wolf
      gets deleted. Preserve that, do not tighten it.
- [ ] TASK-019: `src/core/harness/check.rs` — `learnings.untriaged.<name>.
      <issue>` and `lesson.unenforced.<lesson>` on top of TASK-018's readers.
- [ ] TASK-020: `src/core/harness/check.rs` — widen `lesson.unenforced`'s
      search for a `**Check:**` id from `scripts/` to `scripts/` **and**
      `src/`. Why: RISK-004 — three of the four mechanized lessons name checks
      that land in `src/` in this very phase, so without this the check
      reports its own migration as three errors.
- [ ] TASK-021: `src/cli/commands/harness.rs` — the `Check` action: run
      everything, render findings to stdout in the `doctor` style in human
      mode, and finish an `Envelope<Finding>` with a `summary` carrying
      `plans`, `lessons`, `issues`, `findings`, `error`, `warning`, `info`.
      Why: matching `ash.sh report()`'s summary keys keeps any agent already
      parsing that envelope working.
- [ ] TASK-022: a test comparing `pinst harness check --json` against
      `scripts/ash.sh check --json` on the real corpus, asserting the two sets
      of finding ids are equal. Why: this is the phase's whole claim, and it
      is only runnable while both implementations exist — which is exactly the
      window this phase occupies. Delete the test in phase 5 with the script.

## Trade-offs & risks
RISK-005 is subtle and worth stating on the commit: ids are built from format
strings, so `plan.id-mismatch.{name}` keeps the literal prefix contiguous in
the source and `lesson.unenforced`'s grep can still find it. An id assembled
from fragments (`"plan." + kind + ".id-mismatch"`) would compile, pass every
test, and break `lesson.unenforced` silently.

TASK-022 is a test with a shelf life. It is deliberately written to be deleted
in phase 5 rather than adapted, because after the script is gone there is
nothing left for it to compare against; leaving it behind as a skipped test
would be a fixture with no oracle.

CON-003 holds: nothing here prints a rate, a tally or a suggestion. If a
number would be interesting, it belongs in phase 4.

## Done criteria
- **TEST-013**: TASK-022 passes — both implementations report the same set of
  finding ids on the real corpus.
- **TEST-014**: both exit 0 on the clean corpus, and `pinst harness check`
  prints nothing on stdout in human mode when clean beyond the `corpus clean
  (N plans)` line on stderr.
- **TEST-015**: for each of six deliberately corrupted fixture corpora built
  in a `tempfile::tempdir` — a bad directory name, a duplicate id, a phase-chain
  gap, a mismatched status mirror, an untriaged issue, a mechanized lesson
  naming a nonexistent check — the expected finding id is reported and the exit
  code is 3.
- **TEST-016**: `pinst harness check --json` parses under `python3 -m
  json.tool` and its `summary` carries the same seven keys as `ash.sh check
  --json`.
- **TEST-017**: `grep -rn 'phase.logged-not-done' src/ scripts/` returns a hit
  in `src/`, so `lesson.unenforced` still resolves LESSON-009 after TASK-020.
- **TEST-018**: `just qc` is green with `just harness` still calling the
  script — the port must not depend on the cutover.
