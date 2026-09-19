---
id: 260919-vldfei
slug: self-improving-agent-harness
phase: 4
status: Proposed
---

# Phase 4 — Corroborate with invocation evidence

## Goal
GOAL-004: Add `ash.sh skills --transcripts <dir>`, an opt-in pass that counts
how often each skill was actually invoked — as a `Skill` tool call *and* as a
slash command — and reports it beside the conformance rates, without any part
of `qc` depending on it.

## Why this phase exists
It depends on Phase 3 alone, and comes after it for a reason that is the
central design claim of this plan: the artifact-based report has to stand on
its own first. Built the other way round, the obvious shortcut is to grade
skills by invocation count, and everything downstream becomes tied to one
vendor's private log format (CON-001). Arriving here second, invocation counts
can only ever be a column added to a table that already works without them.

## Steps
- [ ] TASK-018: `scripts/ash.sh` — add `--transcripts <dir>` to the `skills`
      subcommand, defaulting to off. Absent the flag, nothing outside the repo
      is read and the report omits the column entirely rather than printing
      zeros.
      Why: a zero that means "not measured" is the same failure as RISK-003's
      offline zero, and here it would be worse — it would read as "this skill
      is dead" for a skill nobody happened to point the flag at.
- [ ] TASK-019: `scripts/ash.sh` — implement the counter in `python3` (DEP-002)
      over `<dir>/*/*.jsonl`, counting **both** record shapes: `tool_use`
      entries named `Skill` with `input.skill`, and user-content
      `<command-name>/…</command-name>` markers. Map a command to its skill via
      `.claude/commands/<name>.md` where one exists.
      Why: verified while planning, and the single most important detail in
      this phase. Counting only `Skill` tool calls finds
      `{plan-write: 3, conventional-commits: 2, create-pr: 2,
      plan-learnings: 1}` and misses `/cc` and `/pr` entirely — the same skills
      under their other name.
- [ ] TASK-020: `scripts/ash.sh` — emit derived counts only: per-skill
      invocation total and last-seen date. No argument strings, no file paths,
      no quoted text, and nothing written under `.ash/`.
      Why: SEC-001. Those files hold whole conversations from every project on
      the machine, including anything a user pasted. A count cannot leak; a
      sample can, and the moment one lands in the corpus it is in git forever.
- [ ] TASK-021: `scripts/ash.sh` — make every failure in this path non-fatal:
      an unreadable directory, a malformed line, or an unrecognised schema
      degrades that skill's count to `unmeasured` and never changes the exit
      code.
      Why: RISK-004. This reads an undocumented format owned by someone else's
      release cycle. It is allowed to become useless; it is not allowed to
      break the report it decorates.
- [ ] TASK-022: `.agents/README.md` — document the flag, the two record
      shapes, what the counts do and do not prove, and the rule that no check
      may ever depend on them.
      Why: the discrepancy this phase surfaces invites exactly one wrong
      conclusion, and the README is where to pre-empt it.

## Trade-offs & risks
- The motivating anomaly is worth restating: `plan-implement` has written a
  complete run log — `run_start` through `run_end`, 18 `task_done` events —
  and appears **zero** times in either record shape across all 14 transcripts.
  Whatever the cause, it proves invocation counts cannot be the system of
  record. They are a second opinion.
- RISK-004 accepted in full: this is the one part of the plan expected to rot.
  It is isolated behind a flag for that reason.
- SEC-001 accepted with the narrowest possible read: counts and dates only.
- Deferred: no attempt to reconstruct *sessions* or attribute outcomes to
  invocations. Correlating "the skill fired" with "the work went well" needs a
  join key that does not exist, and inventing one would mean writing session
  ids into the corpus — a bigger privacy surface than this phase is worth.

## Done criteria
- TEST-012: `ash.sh skills --transcripts /root/.claude/projects` reproduces
  the counts confirmed while planning, including both record shapes, and shows
  `plan-implement` at zero invocations beside a non-zero artifact conformance.
- TEST-013: `ash.sh skills` with no flag omits the column, reads nothing
  outside the repo (verify with `strace -f -e openat` or by pointing `HOME` at
  an empty directory), and produces the same exit code as before the flag
  existed.
- TEST-014: pointing the flag at a directory of malformed JSONL produces
  `unmeasured`, exit code unchanged, no stack trace.
- TEST-015: the JSON envelope from a `--transcripts` run contains no string
  from any transcript other than skill names and ISO dates (verify by grepping
  the output for a phrase known to appear in a transcript).
