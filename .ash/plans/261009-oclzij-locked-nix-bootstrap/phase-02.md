---
id: 261009-oclzij
slug: locked-nix-bootstrap
phase: 2
status: Done
---

# Phase 2 — Locked package environment

## Goal
GOAL-002: Build the declared CLI environment from committed Nix inputs with no home configuration activation.

## Why this phase exists
Depends only on phase 1. Prove package resolution and source pins before transferring any configuration ownership.

## Steps
Tasks run in listed order, each depending only on its predecessor.
- [x] TASK-005: `flake.nix`, `flake.lock`, `nix/packages.nix` — declare supported Linux/macOS systems, a package-only environment, and a pinst derivation.
- [x] TASK-006: `nix/`, `scripts/bootstrap-nix.sh` — explicitly account for exceptional packages and provide a pinned, verified Nix bootstrap entry point with preflight and dry-run.
- [x] TASK-007: `src/core/environment.rs`, `src/cli/` — add locked environment build/preview handling and expose it through pinst without per-tool Nix installs.

## Trade-offs & risks
Refer to RISK-001 and RISK-003. Use existing binary caches when available; report external packages explicitly instead of making unverified version claims.

## Done criteria
- TEST-003: A disposable Nix container evaluates the supported-system outputs and builds the package environment from `flake.lock` without updating it.
- TEST-004: Preview does not activate or modify home files; the built environment provides declared binaries, and Rust quality checks pass.
