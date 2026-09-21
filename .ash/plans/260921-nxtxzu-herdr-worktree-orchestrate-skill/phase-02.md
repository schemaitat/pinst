---
id: 260921-nxtxzu
slug: herdr-worktree-orchestrate-skill
phase: 2
status: Done
---

# Phase 2 — Herdr topology, handoff and babysitting

## Goal

**GOAL-002:** Complete the Herdr-backed handoff and supervision loop that does
not stop at the first idle state, writing only the portable logs defined in
Phase 1 — still without any plan-corpus or harness-checker dependency.

## Why this phase exists

This phase depends only on Phase 1: the procedure needs a settled log contract
and an explicit harness-agnostic identity before encoding topology and
babysitting. Combining the phases would mix packaging/docs edits with live
Herdr smoke risk; putting this phase first would force inventing event names
while still debating where logs live.

## Steps

- [x] **TASK-005:** `.agents/skills/orchestrate/SKILL.md` — extend the Phase 1
  shell with preflight: `test "${HERDR_ENV:-}" = 1`, `herdr --skill`, installed
  command help, current pane/workspace discovery, `herdr integration status`
  for the selected kind, and `git status` when a git checkout is present.
  Encode the topology decision from REQ-002 — a new worktree workspace for an
  independent writer, a cwd-preserving sibling pane for read-only/non-racing
  work, and escalation when dirty parent content makes neither choice safe.
  Record a missing/outdated optional integration and verify live state
  transitions before relying on them; never install it or add
  `--trust-repository` implicitly. Why: Herdr gate first; git is optional
  context, not a harness.
- [x] **TASK-006:** `.agents/skills/orchestrate/SKILL.md` — after preflight
  chooses a topology, add the isolated handoff: parse Herdr JSON for the
  returned workspace/root-pane or sibling-pane id, verify the delegated cwd,
  select a unique live agent name, and call `herdr agent start` in the
  existing shell pane. For Cursor use `--kind cursor` and pass a caller-named
  model as native `-- --model <model>` arguments; omit the model option when
  none was requested. The initial prompt carries the task boundary, checkout,
  branch/base when known, acceptance criteria, allowed mutations and required
  final report — without requiring a plan id.
- [x] **TASK-007:** `.agents/skills/orchestrate/SKILL.md` — after the child has
  a bounded handoff, add the babysitting state machine. Each wait result is
  followed by `agent get` and `agent read`; `idle`/`done` trigger artifact and
  acceptance verification, not completion; `blocked` is inspected and
  escalated without answering an approval/question UI; `unknown` receives a
  bounded diagnostic wait; timeout and prompt stall are inspected before any
  input. Permit one targeted nudge per distinct missing condition, escalate a
  repeated condition, and write `aborted` when it remains impossible or the
  user declines the needed action. Write `resolved` only after REQ-004 is
  satisfied. Append Phase 1 events throughout, including sanitized prompt
  text and the concrete verification behind `resolved` (REQ-005, REQ-006).
- [x] **TASK-008:** `.agents/commands/orchestrate.md` — after the skill's
  procedure is complete, add `/orchestrate` with the requested task/model as
  arguments and preload `HERDR_ENV`, Herdr version/current pane, selected-kind
  integration status, branch and `git status --short` when available. Query
  the current Herdr pane only after the environment gate passes. Direct the
  runtime to read the skill; do not preload plan index or imply plan
  ownership (REQ-008, CON-004).
- [x] **TASK-009:** `.claude/skills/orchestrate` and
  `.claude/commands/orchestrate.md` — after both canonical assets exist, run
  `just wire` so this checkout projects them (CON-002, LESSON-007). Wiring is
  packaging for pinst, not a runtime requirement stated inside the skill body.
- [x] **TASK-010:** `docs/tools/herdr.toml` — after the skill is wired, add a
  short recipe or `see_also` pointer so `pinst docs` can discover the skill
  without making docs part of the skill's procedure (FILE-006).
- [x] **TASK-011:** `.herdr/orchestrate/<start-time>-<agent-name>.log` — after
  packaging is in place, exercise `/orchestrate` (or direct skill invocation)
  first with a read-only sibling-pane task and then with a disposable mutating
  task that requires a Herdr worktree. Force one bounded wait timeout in the
  disposable run, confirm the supervisor reads state instead of replaying the
  initial prompt, and retain the sanitized logs as evidence. Amend the skill
  before declaring done if either run exposes a wrong command, ambiguous state
  transition or unverifiable resolution rule.

## Trade-offs & risks

The skill is intentionally stricter than a convenience wrapper. REQ-004 means
an agent can be `done` and still receive a targeted follow-up because its
artifact or check is missing (RISK-001).

RISK-002 and RISK-003 shape the two most important stop conditions: a failed
wait is inspected rather than replayed, and a worktree is never assumed to
contain dirty parent changes.

SEC-001 means a blocked approval cannot be automated merely because the
orchestrator can send keys. `blocked` is a reason to capture the UI and
escalate.

RISK-007 is exercised rather than guessed: smoke runs record real transitions.
If they remain `unknown` or blocker detection cannot be trusted, the skill
escalates instead of silently claiming unattended supervision.

The one-nudge policy is keyed by the unresolved condition, not by total prompt
count. RISK-006 keeps every result available for review; this phase does not
add automatic pane, workspace or worktree deletion.

## Done criteria

**Provable in the tree:**

- **TEST-004:** The skill text explicitly covers the `HERDR_ENV` stop,
  worktree-versus-pane choice, dirty-tree stop, JSON/exit-code handling,
  Cursor model forwarding and integration-status handling, all five Herdr
  agent states, no-blind-reprompt recovery, nudge/escalate/abort, semantic
  resolution, portable log path/redaction, preserved cleanup, and
  harness-agnostic non-goals — with no requirement to invoke plan-* skills.
- **TEST-005:** `.claude/skills/orchestrate` and
  `.claude/commands/orchestrate.md` are relative symlinks to the canonical
  assets when this repo's wire step has been run; `just harness` / `just qc`
  remain green for the packaging change alone.
- **TEST-006:** No `src/core/harness/orchestration.rs` (or equivalent) and no
  `.ash/orchestrations/` directory are introduced by this plan.

**Confirmed in live Herdr (requires DEP-001 and DEP-002):**

- **TEST-007:** `herdr --skill`, the relevant `--help` commands,
  `herdr integration status` and Cursor Agent help still expose the
  commands/options documented by the skill; any difference is resolved in the
  skill before smoke testing, and missing integration is recorded without
  mutating the user's installation.
- **TEST-008:** A read-only delegated task uses a sibling pane, returns a
  complete answer, is marked `resolved` only after output verification, and
  leaves a clean portable orchestration log under `.herdr/orchestrate/`.
- **TEST-009:** A disposable mutating task creates an isolated Herdr worktree,
  starts Cursor with an explicitly selected model, produces and verifies its
  requested file/test result, survives a forced wait timeout without duplicate
  submission, records the actual state transitions despite the observed
  integration status, and ends with matching `resolved` and `run_end` events.
  Persistent `unknown` instead produces an escalation rather than false
  resolution. The worktree remains available for inspection until separately
  removed.
