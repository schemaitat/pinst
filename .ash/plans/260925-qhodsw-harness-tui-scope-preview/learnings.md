---
id: 260925-qhodsw
slug: harness-tui-scope-preview
updated: 2026-09-25
areas: [tui, harness, app]
issue_count: 2
---

# Learnings — make the Harness tab scannable and inspectable (260925-qhodsw-harness-tui-scope-preview)

## Source
- Plan: `./README.md`
- Basis: session context
- Logs consulted: `./logs/20260925T055350Z-deepseek-v4.1-flash.log`

## Summary
Both phases landed as planned: the Harness tab now groups project and global
installs under labelled section headers with install roots and per-state
counts, every asset row shows its install path, and a scrollable preview pane
renders the selected asset's source and installed target with explicit empty,
missing, binary, and truncation states. `just qc` is green (332 tests). Two
issues were worth recording: a dirty worktree made the harness check fail
before any code changed, and phase 1 briefly carried phase-2-only state that
clippy's `-D warnings` rejected across the phase boundary.

## Issues

### ISSUE-001: A fresh worktree's stale `.claude` symlinks fail `just harness` before any code runs
**What happened:** `just harness` (and therefore `just qc`) exited 3 with
`wire.broken.orchestrate` before any source was edited. The worktree's
`.claude/skills/orchestrate` and `.claude/commands/orchestrate.md` symlinks
pointed at an absolute path in `feat-orchestrate-skill`, a sibling worktree
that no longer exists.
**Root cause:** `.claude/` is machine-local and untracked, so it is neither
copied from `main` nor validated when a worktree is created; a link created by
an older `pinst harness install` keeps its absolute target forever. The failure
is therefore environmental, not a defect in the branch under test.
**Fix applied:** Repointed both symlinks at this worktree's own `.agents/`
assets with the same relative style as their siblings, then re-ran
`just harness` to confirm the corpus was clean.
**Recommendation:** When `just qc` fails at `harness check` with
`wire.broken.*`, run `readlink` on the named links before suspecting the code.
Absolute paths into another worktree mean stale local wiring; fix the links
(or run the repo's own `pinst harness install --scope project --force`) rather
than weakening or skipping the check.
**Skill:** none
**Distilled:** promoted — LESSON-043.
**Gap:** answered — no skill is missing for `tui`, `harness` or `app`. This is
machine-local wiring drift in an untracked `.claude/`, not work any of those
areas lacks instructions for; the detection is already mechanized
(`wire.broken.*`) and what was missing was the *response*, which is one line in
the skill that creates worktrees rather than a new skill. Added to
`implement-feature` Step 1.

### ISSUE-002: Phase-2-only fields fail phase 1's clippy gate
**What happened:** Phase 1 added `App::harness_source` and
`App::selected_harness` in anticipation of phase 2. `cargo clippy
--all-targets -- -D warnings` rejected both as dead code, so phase 1 could not
pass its own `just qc`.
**Root cause:** The plan's per-phase commit rule says each phase must be
green on its own, but the implementation pulled two symbols forward across the
phase boundary because they were easy to add alongside the phase-1 state model.
Unused fields and methods are exactly what `-D warnings` exists to catch.
**Fix applied:** Deferred both additions to phase 2, where the preview pane
actually reads them; phase 1 then compiled warning-free and was committed.
**Recommendation:** Keep forward declarations out of an earlier phase. If a
later phase needs new `App` state, add it in that phase's commit so each phase
is independently compilable and clippy-clean.
**Skill:** plan-implement
**Distilled:** declined — this is a sequencing slip specific to how this plan
was sliced; the compiler and clippy already enforce it mechanically in the tree,
so no prose lesson would add a check that does not exist.
