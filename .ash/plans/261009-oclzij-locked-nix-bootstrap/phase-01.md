---
id: 261009-oclzij
slug: locked-nix-bootstrap
phase: 1
status: Done
---

# Phase 1 — Reliable legacy execution

## Goal
GOAL-001: Make bootstrap failures honest and prevent dependent work from running against unavailable prerequisites.

## Why this phase exists
This is the first slice: Nix orchestration will rely on the same execution contract, so failure handling must be correct first.

## Steps
Tasks run in listed order, each depending only on its predecessor.
- [x] TASK-001: `src/core/plan.rs`, `src/core/engine.rs` — carry prerequisite step ids, propagate failed/blocked/unauthorized prerequisites transitively, and test independent work continuing.
- [x] TASK-002: `src/core/exec/` — stage remote scripts and releases, preserve failures, support release hashes and pins, bound downloads, and make newly installed user tools visible to later steps.
- [x] TASK-003: `src/core/configs.rs`, `src/cli/commands/apply.rs` — scope config packages to selected tools and gate activation on their prerequisite installs.
- [x] TASK-004: `scripts/install.sh`, regression tests — preserve release pins in source fallback, fail closed after invalid downloads, and verify with disposable fixtures.

## Trade-offs & risks
Legacy installs remain imperative; these changes improve truthful recovery without claiming locked legacy apt/brew dependencies. Refer to RISK-003.

## Done criteria
- TEST-001: Behavioral tests prove dependency failure isolation, downloader/extraction errors, config selection, and pin-respecting source fallback.
- TEST-002: `just qc` passes for this slice.
