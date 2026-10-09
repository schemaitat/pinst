---
id: 261009-oclzij
slug: locked-nix-bootstrap
phase: 3
status: Proposed
---

# Phase 3 — Home Manager ownership and migration

## Goal
GOAL-003: Transfer dotfile ownership explicitly and activate or roll back exact Home Manager generations without losing live edits.

## Why this phase exists
Depends only on phase 2. The package closure is known before generated shell/editor integration and migration introduce home-directory state.

## Steps
Tasks run in listed order, each depending only on its predecessor.
- [ ] TASK-008: `nix/home.nix`, host definitions — immutable config deployment, managed shell integration, runtime identity/secrets, and explicit package/config ownership.
- [ ] TASK-009: `src/core/environment.rs`, CLI — generation activation, status, rollback, recorded migration backups, and protection against competing legacy mutations.
- [ ] TASK-010: migration tests and docs — prove real files and foreign symlinks with live edits are preserved; distinguish generation rollback from pre-migration restoration.

## Trade-offs & risks
Refer to RISK-002 and CON-001. Machine-specific identity stays in a runtime include; no implicit copying of the developer's live values into Git or the store.

## Done criteria
- TEST-005: Disposable-home activation, idempotency and generation rollback pass; old symlink targets and file bytes survive migration.
- TEST-006: Legacy mutations refuse Home Manager-owned state and the source configs retain a single owner.
