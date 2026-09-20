---
id: 260920-tensvp
slug: tool-docs-explorer
phase: 1
status: Done
---

# Phase 1 — The page format and the embedded catalogue

## Goal
**GOAL-001**: settle the shape of a tool page and get the catalogue loading —
from the checkout when there is one, from inside the binary when there is not
— so every later phase reads one already-validated data structure instead of
inventing its own (REQ-004).

## Why this phase exists
The format is the decision everything else is downstream of: search ranks its
fields, `show` renders them, `adopt` writes them, and phase 5 authors 26 files
against them. Getting it wrong is cheap now and expensive after the catalogue
exists (ASSUMPTION-002). Shipping the loader on its own — with one real page
and a test that every page parses and names a manifest tool — proves the
format against the actual embedding mechanism before a single command depends
on it.

## Steps
- [x] TASK-001: `src/core/source.rs` (new) — move `Source`, `resolve_source`,
      `checkout_tree` and `absolute_tree` out of `src/core/configs.rs` into a
      shared module, parameterized by subdirectory and override env var:
      `source::resolve("configs", "PINST_SOURCE")` and
      `source::resolve("docs/tools", "PINST_DOCS_SOURCE")`.
      Why: the docs catalogue wants the identical property configs already
      have — edits in a checkout are live without a rebuild, and a downloaded
      binary still answers (PAT-002). Duplicating thirty lines of path
      canonicalization is how the two quietly stop agreeing.
- [x] TASK-002: `src/core/configs.rs` — delegate to `source::resolve`, keeping
      `resolve_source()` and `Source::describe`/`is_tree` behaviour byte-for-byte
      as they are today. Why: this is a refactor with no intended behaviour
      change; the existing config tests are the proof.
- [x] TASK-003: `src/core/docs/page.rs` (new) — the `ToolDoc` type and its
      parse: `what` (required one-liner), `when`, `keywords`, `status`
      (`draft | authored`), `verified_with`, `recipe[]` of `{ cmd, does }`,
      `gotchas`, `see_also`, and `help` (the raw captured block a draft
      carries). `Serialize` + `Deserialize` + `JsonSchema`, like `Manifest`.
- [x] TASK-004: `src/core/docs/mod.rs` (new) — `load()` reads every `*.toml`
      under the resolved source into a `Catalogue` keyed by tool name, with
      the file stem as the key and a parse error naming the file it came from.
      `static EMBEDDED: Dir = include_dir!("$CARGO_MANIFEST_DIR/docs/tools")`.
      Why `docs/tools` and not `docs/`: `docs/` already holds
      `architecture.svg` and `workflow.svg`, which must not be compiled into
      the binary.
- [x] TASK-005: `docs/tools/ripgrep.toml` (new) — the first real page,
      authored by hand, exercising every field including two recipes and a
      `see_also` to `fd`. Why now: a format with no instance is a guess, and
      the validation that follows needs a real page to chew on.
- [x] TASK-006: `src/core/mod.rs` — register `docs` and `source`.
- [x] TASK-007: `src/cli/mod.rs` + `src/cli/commands/schema.rs` — add
      `SchemaKind::Docs`, emitting `schema_for!(ToolDoc)`. Why: the same
      reason `schema manifest` exists — an agent authoring a page should read
      a schema derived from the parser, not a section of prose that can drift
      (CON-004).
- [x] TASK-008: tests in `src/core/docs/mod.rs` — every embedded page parses;
      every page's stem names a tool in the embedded manifest; every page has
      a non-empty `what` and a legal `status`.
      Why: these are the invariants that can only be true or broken, so they
      belong in `cargo test` where `just qc` already runs them (LESSON-008) —
      unlike coverage, which is a report (GUD-001).

## Trade-offs & risks
- **ASSUMPTION-002** is at its cheapest to reverse in this phase and its most
  expensive after phase 5. If TOML's multi-line strings turn out to fight the
  prose while authoring TASK-005, that is the signal to revisit ALT-004 —
  before four phases of commands and 26 files exist.
- TASK-001/002 are a refactor of code that currently works and is covered by
  tests. Accepted because the alternative is two copies of source resolution
  in a binary whose whole premise is that it behaves the same with and
  without a checkout (RISK-002 is untouched here; DEP-002 arrives with the
  first `include_dir!` of the new tree).
- Nothing user-visible ships in this phase. That is deliberate: the format is
  load-bearing for five phases, and a phase boundary here is what makes it
  reviewable on its own.

## Done criteria
- **TEST-001**: `cargo test` covers, and fails on, each of: a page that does
  not parse, a page naming no manifest tool, a page with an empty `what`, a
  page with an unknown `status`.
- **TEST-002**: `pinst schema docs` emits a schema that validates
  `docs/tools/ripgrep.toml`, and `pinst schema manifest|output` are unchanged.
- **TEST-003**: the catalogue loads from the checkout when `docs/tools/`
  is present and from the embedded copy when it is not — asserted the way
  the config tests already assert the equivalent for `configs/`.
- `just qc` is green.
