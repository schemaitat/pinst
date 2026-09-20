---
id: 260920-wtburh
slug: harness-in-the-binary
phase: 4
status: Done
---

# Phase 4 — `pinst harness skills` — the graded report

## Goal
Port the skill audit: the per-skill conformance table, the gap rows, the
review agenda, and the opt-in invocation counts — the half that measures
rather than gates.

## Why this phase exists
`skills` reads the same issue and lesson tables phase 3 built, so it cannot
precede them without writing that model twice — the reason ALT-008 was
rejected. Keeping it a separate phase from `check` is the same separation the
harness itself insists on: `check` exits 3 on any finding and gates `qc`,
while `skills` reports rates that would fail the build if they were findings.
Landing them together would invite exactly one shared code path with a flag,
which is how the two get confused.

## Steps
- [x] TASK-023: `src/core/harness/evidence.rs` (FILE-009) — run an `evidence:`
      one-liner as `sh -c`, with `ASH_WINDOW` set to the bare number and
      `ASH_RANGE` to `-n <n>` or empty, a 30-second timeout, stderr
      discarded, and a result accepted only if stdout is exactly two integers.
      Why: anything else must report `unmeasured`, never `0` — a zero meaning
      "offline" is worse than a gap that admits it.
- [x] TASK-024: `src/cli/mod.rs` and `src/core/harness/evidence.rs` — a
      `--no-evidence` flag that skips execution and reports every rate as
      `unmeasured`. Why: SEC-001 — this is the one place the binary runs shell
      it took out of a file in the repo being checked, and a trusted installed
      binary is a wider blast radius than a script you had to clone. Document
      it in the module header, not only in `--help`.
- [x] TASK-025: `src/core/harness/skills.rs` (FILE-008) — read each
      `.agents/skills/*/SKILL.md` with phase 1's frontmatter parser, taking
      `produces`, `evidence` and `kind`, and note whether `.claude/skills/
      <name>` exists. Why: the single-quoted YAML scalar with doubled `''`
      escapes is exactly what the strict parser was specified for in TASK-003
      — this is its second consumer and the one that justifies the quoting
      rules.
- [x] TASK-026: `src/core/harness/skills.rs` — grade each skill: run
      `evidence:` twice (windowed at 20, then all-time), compute both rates,
      join the `**Skill:**` tally from every `learnings.md`, and emit
      `skill.no-contract.<name>` and `skill.evidence-failed.<name>` plus
      `skills.missing` — the only findings this action may produce.
- [x] TASK-027: `src/core/harness/skills.rs` — the gap rows: issues attributed
      to `none` and not marked `**Gap:** answered`, grouped by each area their
      plan declares, ordered by count. Why: one issue counts once per area, so
      an area accumulates everything unowned that touched it; that is the
      useful reading and the duplication is intentional.
- [x] TASK-028: `src/core/harness/skills.rs` — the agenda: untriaged issue
      count, skills whose windowed rate is below their all-time rate, the
      largest gap group, and `prose` lessons seen in more than one plan that
      are not marked `**Mechanize:** declined`. Close with `review.agenda`
      at `info` severity when the agenda is non-empty.
- [x] TASK-029: `src/core/harness/transcripts.rs` (FILE-010) — port the
      embedded `python3` scanner to `serde_json`: walk `<dir>/*/*.jsonl`,
      count `Skill` tool calls whose `input.skill` names a known skill, and
      `<command-name>` markers mapped to a skill through
      `.claude/commands/*.md`. Why: DEP-002 drops — `python3` stops being a
      dependency of the harness. Two record shapes, because counting only the
      tool call misses `/cc` and `/pr` entirely.
- [x] TASK-030: `src/core/harness/transcripts.rs` — make every failure in this
      path non-fatal and emit derived counts only: an unreadable directory, a
      malformed line or an unknown schema degrades to `unmeasured` via
      `skills.transcripts-unreadable` and never changes the exit code, and a
      skill name is printed only after matching a directory in
      `.agents/skills`. Why: SEC-002, verbatim from `260919-vldfei` — nothing
      typed into a conversation may reach stdout, and nothing here writes into
      `.ash/`.
- [x] TASK-031: `src/cli/commands/harness.rs` and `src/cli/commands/schema.rs`
      (FILE-015) — the `Skills` action with `--transcripts <dir>`, the human
      table, the `## Gaps` and `## Agenda` sections, an envelope carrying
      `skills` and `gaps` arrays and an `agenda_items` count, and a
      `SchemaKind::Harness` emitting the report item types. Why: `pinst schema
      output` is the machine contract; a payload shape with no schema entry is
      the one part of the envelope an agent cannot discover.

## Trade-offs & risks
SEC-001 is accepted rather than eliminated. `--no-evidence` is an off switch,
not a sandbox: a caller who leaves the default on is running the checked
repo's shell. The alternative — defaulting off — was rejected because a
measurement that must be asked for is a measurement nobody takes, and
ASSUMPTION-003 records that as a default decision the requester may invert.

RISK-005 applies again: `skill.no-contract` and `learnings.untriaged` are both
named by mechanized lessons, so both literals must stay contiguous in the
source.

The transcript port is the largest behaviour-preserving rewrite here and the
one with the least test leverage, since its input is an undocumented format
owned by someone else's release cycle (RISK-004 of the original plan). It
fails soft by construction, and nothing in `qc` depends on it.

## Done criteria
- **TEST-019**: `pinst harness skills` on the real corpus reports the same six
  rows, the same rates and the same issue tallies as `scripts/ash.sh skills`,
  and both exit 0.
- **TEST-020**: `pinst harness skills --json` carries a `skills` array of six
  objects, a `gaps` array, and an `agenda_items` integer, and parses under
  `python3 -m json.tool`.
- **TEST-021**: `pinst harness skills --no-evidence` runs no subprocess
  (asserted by pointing a fixture skill's `evidence:` at a command that would
  create a file, and checking the file does not appear) and reports every rate
  as `unmeasured`.
- **TEST-022**: a fixture skill whose `evidence:` prints one integer, and one
  whose `evidence:` exits non-zero, both produce `skill.evidence-failed.<name>`
  and the rate `unmeasured`, never `0`.
- **TEST-023**: `pinst harness skills --transcripts <fixture dir>` reproduces
  the counts `scripts/ash.sh skills --transcripts` gives for the same
  directory, and `--transcripts /nonexistent` emits
  `skills.transcripts-unreadable` without changing the exit code.
- **TEST-024**: `just qc` is green and `just review` reports the same agenda
  from both implementations.
