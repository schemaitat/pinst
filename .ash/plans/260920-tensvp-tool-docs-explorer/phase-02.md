---
id: 260920-tensvp
slug: tool-docs-explorer
phase: 2
status: Proposed
---

# Phase 2 — Read the catalogue: `docs show` and `docs status`

## Goal
**GOAL-002**: put the catalogue behind the CLI contract — one action that
returns a whole page for a named tool, and one that reports coverage across
the manifest — so the thing is usable by an agent end to end before any of
the cleverness lands (REQ-002, REQ-006).

## Why this phase exists
Phase 1 produced a data structure nobody can reach. The smallest useful
command surface is a lookup by name, and it is also where every contract
decision gets made once: which envelope items a `docs` action emits, which
exit code a tool with no page produces, what human mode prints when `--json`
is absent. Search and capture in later phases then have a settled shape to
conform to rather than each inventing one. Merging this into phase 3 would
mean deciding the contract while also deciding how to run a subprocess
safely — two unrelated arguments in one review.

## Steps
- [ ] TASK-009: `src/cli/mod.rs` — `Commands::Docs(DocsArgs)` with
      `DocsAction::{Show, Status}` for now, plus `name()` returning `"docs"`
      and the dispatch arm. Why the `<command> <action>` shape: `config` is
      the precedent, and a family of five verbs under one noun keeps
      `pinst --help` legible (PAT-001).
- [ ] TASK-010: `src/cli/commands/mod.rs` — `pub mod docs;`.
- [ ] TASK-011: `src/cli/commands/docs.rs` (new) — `show <tool>`: resolve the
      tool against the manifest, load its page, emit an envelope whose item is
      the page plus the probed `installed`/`version`. Human mode renders the
      page as a compact block — `what`, `when`, then recipes as
      `cmd` / `does` pairs — on stdout, with the manifest source note on
      stderr as every other command does.
- [ ] TASK-012: `src/cli/commands/docs.rs` — the exit-code contract for
      `show`: unknown tool name is exit 2 (usage, as an unknown tool in a
      selection already is); a known tool with no page is exit 3 with an
      `errors[]` entry naming the remediation (`pinst docs adopt <tool>`, once
      phase 3 exists); a page found is exit 0.
      Why 3 and not 1: nothing failed — the command ran correctly and found
      something to act on, which is exactly what 3 means in this CLI.
- [ ] TASK-013: `src/cli/commands/docs.rs` — `status`: one item per manifest
      tool with `page` (`authored | draft | none`), `installed`, `version`,
      `verified_with`, and a summary counting each. Exit 0 always.
      Why always 0: coverage is a report, and a report that fails the build
      on the normal state gets switched off (GUD-001, LESSON-011).
- [ ] TASK-014: tests — `show` on a tool with a page, on a tool without one,
      and on a name that is not in the manifest, asserting the exit code and
      the envelope of each; `status` over a fixture manifest asserting the
      per-tool classification and the summary counts.

## Trade-offs & risks
- `status` deliberately has no `--fix` and no doctor integration (ALT-008).
  The cost is that nothing nags about an empty catalogue; the benefit is that
  `just qc` stays green and therefore stays trusted while phase 5 is
  outstanding.
- **RISK-003** first becomes real here: `show` will happily return a `draft`
  page. The mitigation is that `status` travels in the item and human mode
  prints it next to the name, so a draft is never silently presented as
  verified.
- `show` reports `installed: false` rather than refusing. A page is useful
  before the tool is there — deciding whether to install it is one of the
  questions the catalogue exists to answer.

## Done criteria
- **TEST-004**: the three `show` exit codes (0 / 2 / 3) are each asserted,
  and the exit-3 case carries a remediation string.
- **TEST-005**: `pinst docs status --json` classifies every manifest tool and
  its summary counts add up to the manifest's tool count; the command exits 0
  with an empty catalogue.
- `pinst docs show ripgrep` prints a readable page on a terminal and a valid
  envelope under `--json`, with nothing but the source note on stderr.
- `just qc` is green.
