---
id: 261009-oclzij
slug: locked-nix-bootstrap
status: In Progress
created: 2026-10-09
updated: 2026-10-09
areas: [bootstrap, exec, nix, configs]
summary: Harden bootstrap execution and provide a locked Nix and Home Manager environment with explicit migration and rollback.
files_touched: [src/core/engine.rs, src/core/exec, src/core/configs.rs, src/core/environment.rs, src/cli, scripts/install.sh, flake.nix, flake.lock, nix, e2e, docs/bootstrap.md]
---

# Stable bootstrap with Nix and Home Manager

## Context
REQ-001: Bootstrap must report real failures, respect dependencies, and install the requested revision. REQ-002: A committed lock must identify the user toolchain and configuration rather than resolving latest on each machine. REQ-003: Configuration migration must preserve existing files and live dotfile edits, with an independently verifiable rollback path. REQ-004: Shell and editor readiness must be tested alongside inventory.

The source checkout is clean at `307631835a9e599cfe0c2f508773c10a89928530`. Implementation uses `/home/andre/.herdr/worktrees/pinst/feat-locked-nix-bootstrap`, branch `feat/locked-nix-bootstrap`. The active `~/dotfiles` repository contains uncommitted shell and Herdr edits; they are user data, not a source checkout to commit or silently copy into this feature.

## Decision
First repair the existing execution path, then introduce a locked flake with a package-only pilot and standalone Home Manager configurations. Add an explicit pinst environment interface for building, activating, inspecting and rolling back exact generations. Keep the existing config files canonical where possible, and supply Nix-specific shell/editor integration where immutable packaging requires it. Migration records and preserves existing live files before transferring ownership; legacy mutations must not fight a Home Manager-owned home. Validate Nix in a disposable Docker environment rather than activating it on the developer's live account.

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Per-tool `nix profile install nixpkgs#tool` | Retains moving inputs and competing package ownership. |
| ALT-002: Rebuild dependency locking and generations in pinst | Nix and Home Manager already provide the package closure and generation semantics. |
| ALT-003: Immediately replace the live home | Would silently choose between uncommitted dotfile edits and the repository defaults. |

## Consequences
CON-001: No host package installation, live activation, publication, or destructive cleanup during implementation. DEP-001: Docker is available for Linux Nix evaluation/build/activation tests; macOS execution belongs in CI. RISK-001: Nixpkgs OpenCode is V1 while this manifest requires V2; exceptional packages must have explicit pinned definitions or remain explicitly reported as external, never silently substituted. RISK-002: Home Manager rollback covers its generation, not arbitrary external state or secret material. GUD-001: Keep secrets out of store inputs and preserve native runtime config languages. RISK-003: Binary cache/network failures must remain visible; a lock alone does not guarantee availability.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Reliable legacy execution | [phase-01.md](phase-01.md) | Done |
| 2 | Locked package environment | [phase-02.md](phase-02.md) | Done |
| 3 | Home Manager ownership and migration | [phase-03.md](phase-03.md) | Done |
| 4 | Readiness and delivery verification | [phase-04.md](phase-04.md) | Proposed |

## Affected Files
- FILE-001: `src/core/engine.rs`, `src/core/plan.rs`, `src/core/exec/` — dependencies, failures, staged verified artifacts and execution environment.
- FILE-002: `src/core/configs.rs`, `src/cli/commands/apply.rs`, `scripts/install.sh` — selected configs and pinned entry point.
- FILE-003: `flake.nix`, `flake.lock`, `nix/` — locked package and home configurations.
- FILE-004: `src/core/environment.rs`, `src/cli/` — generation lifecycle and ownership.
- FILE-005: `scripts/`, `e2e/`, `.github/workflows/ci.yml`, `justfile`, `docs/bootstrap.md`, `README.md` — installer, regression checks, migration and operating contract.

## Open Questions
None for repository implementation. Activating on the live account and publishing the branch require a later explicit user action.
