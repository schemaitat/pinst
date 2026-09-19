---
id: 260918-lmmebj
slug: self-contained-toolchain-cli
phase: 1
status: Done
---

# Phase 1 — Manifest schema + core domain model

## Goal
GOAL-001: Replace `registry.toml` with a `manifest.toml` that is the single
source of truth for tools *and* configs — carrying dependency edges, tags, and
per-method install/upgrade details — behind a validated `core::` domain model,
so every later phase reads one schema instead of inventing its own.

## Why this phase exists
Nothing else can be built first: the CLI contract (Phase 2) serializes these
types, the engine (Phase 3) orders work from their `requires` edges, and the
config subsystem (Phase 4) reads their `[[config]]` entries. Today's
`ToolSpec` has no dependency edges, no tags, and no config concept, so
starting anywhere else would mean building against a schema that is about to
change underneath it. This phase has no predecessor.

## Steps
- [x] TASK-001: `manifest.toml` — author the manifest superseding
      `registry.toml`: `[meta] schema_version`, `[[tool]]` entries (name,
      summary, tags, `requires`, `[tool.detect]` check/version command +
      regex, `[tool.install]` method + method fields, optional
      `[tool.upgrade]` override, optional post-install steps flagged
      `confirm = true`), `[[config]]` entries, and `[profile.*]` tag sets.
      Why: the `requires` edges encode ordering constraints that are implicit
      and unenforced in `bootstrap.sh`'s hand-written step order (zsh → omz →
      plugin, rust → tree-sitter-cli, nvm → node, uv → just).
- [x] TASK-002: `src/core/manifest.rs` — serde + schemars types for the above,
      plus the loader (`PINST_MANIFEST` env → `~/.config/pinst/manifest.toml`
      → `./manifest.toml` → the `include_str!` embedded copy) and validation
      (duplicate tool names, `requires` naming an unknown tool, unknown
      platform key, config entry referencing an unknown tool).
      Why: the layered loader is carried over from the existing registry
      loader so an operator can override the manifest without rebuilding,
      while the embedded copy keeps the binary self-contained (CON-002).
- [x] TASK-003: `src/core/graph.rs` — topological ordering over `requires`
      with cycle detection, and profile/tag selection narrowing the tool set.
      Why: profiles are what let one manifest drive both "bootstrap a new box"
      and "keep this box in sync" (REQ-006).
- [x] TASK-004: `src/core/mod.rs` + delete `src/registry/mod.rs` — introduce
      the `core::` root and rewire `src/probe/`, `src/upgrade/`, and
      `src/app.rs` onto `core::manifest` types so the TUI keeps building
      (CON-003).
- [x] TASK-005: `src/core/manifest.rs` + `src/core/graph.rs` — unit tests for
      parse, validation rejections, topological order, and cycle detection.

## Trade-offs & risks
- CON-001/ASSUMPTION-001: the schema accommodates per-platform install
  overrides but only the apt/Linux path is implemented; an unsupported
  platform must produce a clear error rather than a silent wrong install.
- RISK-007 applies from here on: this phase deletes `src/registry/` and must
  leave `cargo build` and the TUI working in the same commit.
- Deferred: no manifest migration tooling from `registry.toml` — the file is
  rewritten once, by hand, and the old one deleted.

## Done criteria
- TEST-001: unit tests cover manifest parse, each validation rejection,
  topological ordering against the known constraints (zsh before oh-my-zsh,
  rust before tree-sitter-cli, nvm before node), and cycle detection; the
  shipped `manifest.toml` loads and validates clean; `cargo build` and the
  existing TUI still work.
