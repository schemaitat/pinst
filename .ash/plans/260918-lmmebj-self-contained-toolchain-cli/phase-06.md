---
id: 260918-lmmebj
slug: self-contained-toolchain-cli
phase: 6
status: Done
---

# Phase 6 — Distribution, cutover, agent docs

## Goal
GOAL-006: Ship it as the self-contained artifact the whole plan is for — a
small release binary, a fresh-machine install one-liner, a `pinst bootstrap`
one-shot — and complete the endgame: document the `~/dotfiles` cutover and
write the agent-facing contract that makes the tool usable without reading
Rust source.

## Why this phase exists
Everything here depends on Phase 5's `apply`/`doctor` existing: `bootstrap` is
their composition, and the cutover procedure is only safe to document once
`doctor` can verify the result is green. Keeping it as its own phase means the
release profile, the install script, and the migration each get deliberate
attention instead of being tacked onto the end of a feature slice.

## Steps
- [x] TASK-028: `Cargo.toml` — release profile tuned for a small
      self-contained binary (LTO, symbol stripping, single codegen unit).
- [x] TASK-029: `src/cli/commands/bootstrap.rs` — fill the Phase 2 stub: one
      command that installs the selected profile's tools, applies configs, and
      finishes with a doctor report, honoring `--dry-run`/`--yes`/`--json`.
      Why: REQ-005 — a new machine should be one downloaded binary and one
      command, with no clone and no `just`/`stow` prerequisites.
- [x] TASK-030: `scripts/install.sh` — the fresh-machine entry point that
      places the binary on `PATH` and points at `pinst bootstrap`.
- [x] TASK-031: `AGENTS.md` — the agent contract: every command, the JSON
      envelope shape, the exit-code table, `--dry-run`/`--yes` semantics, and
      how to extend the manifest (with `pinst schema manifest` as the
      authority).
      Why: REQ-007 — "usable by AI agents" is a documentation deliverable as
      much as a code one; an agent that must infer the contract will get it
      wrong.
- [x] TASK-032: `README.md` — human docs plus the explicit `~/dotfiles`
      cutover procedure: apply configs with pinst (re-pointing `$HOME` links
      away from `~/dotfiles`), verify with `doctor`, then archive the old repo
      read-only rather than deleting it (RISK-005, ASSUMPTION-004).
- [x] TASK-033: end-to-end verification — `bootstrap --dry-run` against a
      temporary `$HOME`, the release binary running standalone with no source
      tree present, and `pinst tui` still working.

## Trade-offs & risks
- RISK-005 is resolved here and nowhere else: the cutover is documented as
  re-point → verify → archive, never delete-first, because `$HOME`'s shell
  configs currently symlink into `~/dotfiles` and breaking them breaks the
  login shell.
- ASSUMPTION-002: the dotfiles content moved in Phase 4 is authoritative; the
  archived repo is a safety net, not a second source of truth — once cutover is
  done, edits go through this repo.
- Deferred: no cross-compilation, release automation, or package-manager
  distribution; a locally built binary plus the install script is the scope.

## Done criteria
- TEST-006: `pinst bootstrap --dry-run --json` against a temporary `$HOME`
  emits a complete, ordered plan covering tools and configs; the release binary
  runs standalone from a directory with no source tree and still applies
  configs from its embedded copy; `pinst tui` launches; `AGENTS.md` documents
  every shipped command, the envelope, and the exit codes.
