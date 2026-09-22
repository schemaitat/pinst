---
id: 260921-nxtxzu
slug: herdr-worktree-orchestrate-skill
phase: 1
status: Done
---

# Phase 1 — Portable skill contract and log format

## Goal

**GOAL-001:** Define the harness-agnostic skill shell and the portable
orchestration log schema inside the skill itself, so later Herdr procedure
work writes against a documented contract that does not depend on `.ash/` or
`pinst harness check`.

## Why this phase exists

This is the first phase and has no predecessor. The log and skill identity
must be settled before encoding the Herdr state machine; otherwise Phase 2
would invent event names while still arguing about whether the artifact lives
in the plan corpus. Keeping format-in-skill (GUD-001, CON-004) also prevents a
Rust checker from becoming the de facto owner of a procedure that must run
outside this repo.

## Steps

- [x] **TASK-001:** `.agents/skills/orchestrate/SKILL.md` — create the skill
  with trigger-focused frontmatter (`name`, `description`) that mentions Herdr
  worktree/pane orchestration and babysitting to resolution. State explicitly
  that the skill is harness-agnostic: no `.ash/`, plan id, or plan-* skill is
  required. Omit `produces`/`evidence` harness-contract fields unless they can
  be expressed without referring to the plan corpus; prefer a short
  "Artifacts" section in the body instead. Why: CON-004 / REQ-007 — frontmatter
  that grades `.ash/` would re-couple the skill to the harness.
- [x] **TASK-002:** `.agents/skills/orchestrate/SKILL.md` — after the identity
  paragraph exists, document the portable log path (default
  `.herdr/orchestrate/<YYYYMMDDTHHMMSSZ>-<agent-name>.log` relative to the
  orchestrator cwd, with an override note) and the JSONL envelope: `ts`,
  `event`, `run`, nullable `workspace`/`pane`/`agent`/`worktree`, `message`,
  and object-valued `detail`. Define the vocabulary `run_start`,
  `pane_created`, `worktree_created`, `agent_started`, `prompt_sent`,
  `wait_result`, `blocked`, `nudge`, `escalated`, `resolved`, `aborted`, and
  `run_end`. State append-only rules and that `resolved`/`aborted` require
  verification/reason before `run_end` (REQ-006, PAT-001, SEC-002).
- [x] **TASK-003:** `.agents/skills/orchestrate/SKILL.md` — once the schema is
  written, add the non-goals block: not a plan-lifecycle stage; does not
  write plan checkboxes, `.ash/CHANGELOG.log`, or call `plan-implement`; does
  not register findings with `pinst harness check`; may be invoked with or
  without a git repo as long as Herdr preflight passes (REQ-007, CON-004).
- [x] **TASK-004:** `.agents/README.md` — after the skill draft exists, add a
  short routing note that `/orchestrate` (when present) is a Herdr helper
  outside the plan-write → implement → learnings diagram, and point readers at
  the skill for the real procedure. Do not add orchestration logs to the
  harness artifact tables.

## Trade-offs & risks

Skipping a harness checker means orphaned logs are an operational concern
rather than a `just qc` finding (RISK-005). That is deliberate under CON-004:
a check that only this repo can run would make the skill look harness-owned.

ASSUMPTION-001 (default `.herdr/orchestrate/` path) is recorded rather than
verified on every host; Phase 2's smoke run confirms the default is writable
in this environment and documents override wording if not.

## Done criteria

- **TEST-001:** The skill file exists under `.agents/skills/orchestrate/` and
  states harness-agnosticism, the default log path, the full event vocabulary,
  and the non-goals block in plain language a foreign checkout can follow.
- **TEST-002:** `.agents/README.md` does not list orchestrate as a peer of
  plan-write / plan-implement / plan-learnings in the lifecycle diagram; any
  mention is as an optional Herdr helper.
- **TEST-003:** No new `src/core/harness/*` orchestration module or
  `.ash/orchestrations/` path appears in this phase's diff.
