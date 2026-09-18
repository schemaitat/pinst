# Phase 3 — Dotfiles Health view

## Status
Done

## Goal
GOAL-003: Check Stow symlink integrity per dotfiles package (`nvim`, `zsh`,
`herdr`, `git`) and cross-reference registry tools' PATH presence, flagging
missing/broken/conflicting state — the health-check requirement (REQ-004).

## Why this phase exists
Health checking depends on Phase 2's `ToolSpec`/`ProbeResult` for its PATH-
presence half (TASK-012 reuses Phase 2's probe results instead of re-running
subprocess checks), so it cannot come before the registry/probing phase. It
comes before the editor phase because Phase 4's file picker (TASK-015) reuses
this phase's package/file enumeration (TASK-011) rather than duplicating it.

## Steps
- [x] TASK-011: `src/health/mod.rs` — enumerate `~/dotfiles` packages (read the
      justfile's `packages` var, or mirror it: `nvim zsh herdr git`) and, for
      each tracked file, compute the expected `$HOME` target and classify it:
      `Ok` (symlink → repo), `Missing`, `Broken` (dangling symlink), or
      `Conflict` (plain file present, not a symlink, and differing from the
      repo's tracked version).
      Why: RISK-004 — only flag `Conflict` when the plain file actually
      differs from the repo version, so files adopted via `just adopt` (which
      are legitimately plain files matching the repo) are never
      false-positived.
- [x] TASK-012: `src/health/mod.rs` (cont.) — cross-check each registry
      `ToolSpec`'s `check_cmd` binary against PATH via the `which` crate
      (DEP-006), reusing Phase 2's `ProbeResult`s where a tool was already
      probed instead of re-running the subprocess check.
      Why: avoids duplicating Phase 2's subprocess calls; Health is presented
      as a complementary "is dotfiles state consistent" summary, not a second
      tool list.
- [x] TASK-013: `src/ui/health.rs` — render per-package symlink status plus the
      PATH-check summary, color-coded by `health::Status`; wire in as the
      Health tab.

## Trade-offs & risks
- RISK-004 is addressed directly (diff-based conflict detection); the
  trade-off is one extra file read (repo version vs. target) per tracked file,
  accepted since the package/file count is small (~20 files across 4
  packages).
- This phase does not attempt to *fix* health issues (e.g. auto-run `just
  install`) — it only reports them, consistent with the plan's read-only/
  detection-focused decision (see README `## Decision`).

## Done criteria
- TEST-005: Health tab correctly flags a deliberately broken symlink (e.g.
  `rm ~/.zshrc` while `~/dotfiles/zsh/.zshrc` remains) as `Broken`, and a
  deliberately created plain file at a Stow target as `Conflict`, then clears
  both after `just install`/removing the interfering file.
