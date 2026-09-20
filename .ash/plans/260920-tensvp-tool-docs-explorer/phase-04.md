---
id: 260920-tensvp
slug: tool-docs-explorer
phase: 4
status: Done
---

# Phase 4 — The one-call surface: `docs search` and `docs dump`

## Goal
**GOAL-004**: answer the question an agent actually has — "which tool, and
what do I type" — in a single command, and offer the whole catalogue as one
document for an agent that would rather not call anything again (REQ-001,
REQ-003).

## Why this phase exists
This is the phase the plan exists for; everything before it was the substrate.
It comes last of the mechanical phases because ranking needs something to
rank: the page fields from phase 1, the manifest metadata phase 2 joined to
them, and the captured text phase 3 produces. Building search first would have
meant ranking a catalogue of one.

## Steps
- [x] TASK-022: `src/core/docs/search.rs` (new) — case-insensitive scored
      matching over tool name, manifest `summary` and `tags`, and the page's
      `what`, `when`, `keywords` and each `recipe.cmd`/`recipe.does`. Rank:
      exact name, then name substring, then keyword, then recipe text, then
      prose. Ties break on tool name so the order is stable.
      Why hand-rolled: 26 entries, no new dependency in a size-tuned binary,
      and a ranking whose every rule can be read off the source (ALT-007).
- [x] TASK-023: `src/cli/commands/docs.rs` — the `search <query>` action. Each
      item carries the tool, its score, the field that matched, the page's
      `what`, install status and version, and **the matching recipes inline**.
      Why inline recipes: the whole point is that one call is enough to act;
      a result set that only says "see `docs show rg`" has moved the work, not
      done it.
- [x] TASK-024: `src/cli/commands/docs.rs` — `--tag`, `--limit` and
      `--installed` filters on `search`. No hits is exit 0 with empty
      `items[]`, not exit 3.
      Why 0: an empty result is a correct answer, and making it 3 teaches
      every caller to write retry logic around a working command.
- [x] TASK-025: `src/cli/commands/docs.rs` — the `dump` action: every page in
      one payload, `--json` as an array and human mode as one Markdown
      document, with `--tag`/`--profile` to narrow it. Capture never runs here
      (SEC-001), so `dump` is offline, deterministic and safe to pipe into a
      context window.
- [x] TASK-026: tests — ranking assertions (an exact name outranks a keyword
      hit outranks a prose hit; ties are stable), the empty-result exit code,
      filter behaviour, and a `dump` test asserting every page in the
      catalogue appears exactly once.

## Trade-offs & risks
- Substring matching will miss a synonym nobody wrote down. The mitigation is
  the page's `keywords` field, which exists precisely so that authoring can
  fix a miss — and phase 5 is where the misses get found, by querying the
  catalogue with the intentions an agent actually has.
- `dump` grows linearly with the catalogue (**RISK-002** and **RISK-004** in
  one place). 26 short pages is a small document; the `--tag`/`--profile`
  filters exist so it does not have to stay small forever.
- Ranking is a heuristic with no test that can prove it is *good*, only tests
  that pin it against regression. Accepted: the alternative is a scoring model
  nobody can explain or reproduce.

## Done criteria
- **TEST-008**: the ranking order, the stable tie-break, the exit-0 empty
  result and each filter are covered by unit tests.
- **TEST-009**: `pinst docs dump --json` contains every page exactly once,
  carries `schema_version` 1, and runs with no subprocess executed (asserted,
  not assumed).
- `pinst docs search "search a tree" --json` returns `ripgrep` first on the
  authored page from phase 1, with its recipes inline.
- `just qc` is green.
