---
id: 261009-oclzij
slug: locked-nix-bootstrap
phase: 4
status: In Progress
---

# Phase 4 — Readiness and delivery verification

## Goal
GOAL-004: Verify a usable locked shell/editor environment and document the complete bootstrap, update, migration and rollback workflow.

## Why this phase exists
Depends only on phase 3. Readiness must exercise the activated generation rather than just inspecting package definitions.

## Steps
Tasks run in listed order, each depending only on its predecessor.
- [x] TASK-011: `nix/` editor integration — package plugins, parsers and language servers together; eliminate required first-start network provisioning in the Nix editor.
- [x] TASK-012: `e2e/`, CI, `justfile` — automated fresh-home, repeat-run, rollback, shell and headless editor checks using the same pinned environment.
- [x] TASK-013: `docs/bootstrap.md`, `README.md`, pinst contract — document installation, exception ownership, upgrade/pin workflow and migration recovery; finish `just qc` and record all verification evidence.

## Trade-offs & risks
Linux activation is locally provable using DEP-001. The CI matrix defines macOS execution coverage; this Linux session cannot claim those jobs already passed.

## Done criteria
- TEST-007: Locked container build/activation and offline shell/editor smoke checks pass, including parser and language-server readiness.
- TEST-008: `just qc` passes; plan statuses, phase commits, run closure and learnings are consistent. Documentation contains executable commands matching the implemented CLI.
