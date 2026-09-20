---
id: 260920-juwako
slug: mechanize-lesson-renumbering
phase: 2
status: Proposed
---

# Phase 2 — Point the finding at the remedy, and close LESSON-022's own gap

## Goal
Make `lesson.duplicate-id`'s remediation name the command phase 1 just
built instead of "grep the corpus by hand," and update LESSON-022's own
text to say the remedy is mechanized now too — closing the exact gap it
named, in the file where it named it.

## Why this phase exists
Phase 1's command has no reason to exist in the corpus's own eyes until
something points at it: the finding a person actually sees
(`lesson.duplicate-id`) still tells them to renumber by hand, and
LESSON-022 still reads as half-fixed. Both edits only make sense once the
command exists, which is why they wait for phase 1 rather than landing
alongside it.

## Steps
- [ ] TASK-001: `src/core/harness/check.rs` — change `lesson.duplicate-id`'s
      remediation string from "renumber the later one to the next free
      LESSON-NNN and grep the corpus for citations of it" to point at
      `pinst harness renumber-lesson --title "<its title>"`. Update the
      fixture test's expectations if any assert the old remediation text
      verbatim.
      Why: this is the finding a person actually reads when the collision
      happens; it is the highest-value place to name the new command.
- [ ] TASK-002: `.ash/LEARNINGS.md` — extend LESSON-022's `Lesson`/`Why`
      text to record that the remedy, not only the detection, is mechanized
      now (naming `pinst harness renumber-lesson`). `**Status:**
      mechanized` and `**Check:** lesson.duplicate-id` stay as PR #17 left
      them — this only adds to the prose, since the check that makes the
      lesson "mechanized" is still the same finding.
      Why: depends on TASK-001 — the lesson should describe the remedy as
      it actually reads in the check's own remediation text, not a
      paraphrase written before it existed.
- [ ] TASK-003: Run `just qc` and `just harness` (TEST-007) and confirm
      clean.
      Why: depends on TASK-002 — nothing to verify until both edits land.

## Trade-offs & risks
None beyond what phase 1 already carries.

## Done criteria
- [ ] TEST-007: `pinst harness check`'s `lesson.duplicate-id` remediation
      text names `renumber-lesson`; `the_real_corpus_is_clean` and
      `every_mechanized_lesson_resolves_to_a_literal_in_the_source` both
      still pass against the updated `.ash/LEARNINGS.md`; `just qc` is
      green.
