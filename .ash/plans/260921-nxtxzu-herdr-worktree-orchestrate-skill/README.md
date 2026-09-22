---
id: 260921-nxtxzu
slug: herdr-worktree-orchestrate-skill
status: Done
created: 2026-09-21
updated: 2026-09-21
areas: [agents, herdr, orchestration]
summary: Add a harness-agnostic orchestrate skill that delegates through Herdr, babysits agents to semantic resolution, and writes a portable orchestration log.
files_touched: [.agents/skills/orchestrate/SKILL.md, .agents/commands/orchestrate.md, .claude/skills/orchestrate, .claude/commands/orchestrate.md, .agents/README.md, docs/tools/herdr.toml]
---

# Herdr worktree orchestration as a portable skill

## Context

Agents already drive Herdr panes and worktrees ad hoc, but there is no reusable
procedure for creating the right topology, handing a task to another coding
agent, and staying responsible after the first wait returns. Herdr exposes the
primitives (worktree/workspace/pane creation; agent start, prompt, wait, read,
get; machine-readable lifecycle state). Without a skill that joins them, each
caller improvises topology, completion semantics and recovery, and an `idle` or
`done` transport state is easily mistaken for a resolved task.

The skill must be **harness-agnostic**: it depends on Herdr and the delegated
agent runtime, not on this repository's plan corpus, `.ash/` layout,
`plan-write` / `plan-implement` / `plan-learnings` lifecycle, or `pinst harness
check`. It may be *shipped* from `.agents/skills/` like other skills here, but
its procedure must work in any Herdr session — including repos with no `.ash/`
and sessions that never open a plan.

Planning verified the installed interfaces rather than inferring them
(LESSON-002). `herdr 0.9.1`, `herdr --skill`, the relevant `herdr --help`
groups, and live read-only list/current commands confirm that control is gated
by `HERDR_ENV=1`, API commands return JSON with opaque workspace/pane ids,
agent states are `idle`, `working`, `blocked`, `done` and `unknown`, and server
errors and syntax errors exit 1 and 2 respectively. Cursor's installed agent
help confirms that `herdr agent start <name> --kind cursor --pane <id> --
--model <model>` is the supported model-selection path. `herdr integration
status` also confirms that the optional Cursor lifecycle integration is not
installed on this machine, so the skill cannot assume integration-assisted
state reports merely because Cursor itself is available.

- **REQ-001:** Add an `orchestrate` skill that creates the right Herdr topology,
  starts a requested Cursor or other supported agent, hands it a bounded task,
  and remains responsible until that task is semantically resolved or
  explicitly aborted.
- **REQ-002:** Choose a dedicated Herdr worktree workspace for mutating,
  independently reviewable work, and a sibling pane in the current workspace
  for read-only investigation, test running or other work that cannot race a
  writer. A task that depends on uncommitted parent changes must stop for a
  topology decision rather than silently creating a worktree from stale
  `HEAD`.
- **REQ-003:** Drive `herdr worktree`, workspace/pane discovery, and
  `herdr agent start|prompt|wait|read|get` through their JSON responses,
  preserving returned ids and the delegated checkout path instead of deriving
  either from labels or screen order.
- **REQ-004:** Treat Herdr state as transport state, not task outcome. A
  delegated task is resolved only when its response addresses the request,
  its promised artifact exists, its acceptance checks pass, and no blocker is
  outstanding.
- **REQ-005:** Recover from `blocked`, `unknown`, prompt stalls and timeouts by
  inspecting `agent get` and `agent read` first, then choosing a
  state-specific wait, nudge, escalation or abort. Never resend the original
  prompt merely because a wait failed.
- **REQ-006:** Write one append-only JSONL log per orchestration to a portable
  path owned by the skill (default under the orchestrator's cwd or the
  delegated worktree — e.g. `.herdr/orchestrate/<start-time>-<agent-name>.log`),
  never under `.ash/` and never requiring a plan corpus to exist.
- **REQ-007:** Keep the skill free of plan-lifecycle ownership. It does not
  tick plan checkboxes, write `.ash/CHANGELOG.log`, or require
  `plan-implement`. If a caller happens to delegate plan work, that is opaque
  task content for the child — not a special case the orchestrator encodes.
- **REQ-008:** Ship an optional `/orchestrate` command in this repo as a
  convenience entry point that preloads Herdr/pane/git context. Document that
  it is **not** a harness lifecycle stage and does not belong in the
  plan-write → plan-implement → plan-learnings chain.
- **SEC-001:** Refuse all Herdr inspection or control unless
  `HERDR_ENV=1`; do not use `--trust-repository`, answer an agent's approval
  UI, install an agent integration, or control an unrelated pane/workspace
  without explicit authorization.
- **SEC-002:** Logs may retain delegated prompts for auditability but must
  redact credentials, tokens and other secrets before appending them; a
  secret-bearing prompt is summarized with the omitted fields named.
- **CON-001:** Treat Herdr's stdout as JSON data and its documented exit
  statuses as control flow. Do not parse human terminal rendering or infer
  that a failed connection means a mutation did not happen.
- **CON-002:** In this repository, `.agents/skills/` and `.agents/commands/`
  remain the canonical sources and are projected into `.claude/` with
  `just wire` (LESSON-007). That is packaging for pinst — not a runtime
  dependency of the skill's procedure.
- **CON-003:** Phases and tasks remain strictly linear, and `just qc` is the
  repository quality gate for whatever this repo changes while shipping the
  skill.
- **CON-004:** The skill is harness-agnostic. No step may require `.ash/`,
  `pinst harness check`, plan ids, or any plan-* skill. Optional coexistence
  with this repo's harness is packaging and documentation only.
- **GUD-001:** Prefer documenting the log schema inside the skill itself so
  any consumer can validate or ignore the file without a pinst binary.
- **PAT-001:** Orchestration logs use UTC-sortable filenames, compact JSON
  Lines, an explicit event vocabulary, append-only writes, and a terminal
  `run_end` — a portable convention, not a harness-enforced corpus invariant.

## Decision

Ship a **Herdr-only** orchestration skill: topology selection, agent handoff,
babysitting to semantic resolution, and a portable append-only JSONL log.
Do not add a second harness lifecycle, `.ash/orchestrations/` corpus, or
`pinst harness` checker for these logs. Enforcement of log shape stays in the
skill's instructions and its own smoke evidence; this repo does not grow a
Rust module whose only job is to grade orchestration runs (CON-004).

The skill selects topology from the task's write isolation needs. A mutating
task receives a new Herdr worktree workspace and root pane based on a committed
ref; a read-only or non-racing auxiliary task receives a sibling pane with its
cwd preserved. It parses creation responses for the ids used by
`agent start`, passes a requested Cursor model after `--`, and omits a model
flag when none was requested rather than guessing an account-specific name.
Preflight records `herdr integration status` for the selected kind. A missing
optional integration is never installed automatically or silently treated as
fully capable: the first live state transitions decide whether observation is
adequate, and persistent `unknown` or unreliable blocked-state detection moves
to explicit user escalation.

Supervision is a state machine rather than one `prompt --wait`. Every settled
state triggers `get`/`read` and acceptance verification. An unresolved
`idle`/`done` result receives one targeted nudge for the specific missing
condition; `blocked` is inspected and escalated without answering for the
user; persistent `unknown`, repeated stalls, or the same unresolved condition
after a nudge are escalated and then aborted if they cannot be cleared.
`resolved` is written only after the supervisor independently checks the
response and artifacts. Worktrees and panes are preserved for review on both
resolution and abort unless the user separately authorizes cleanup.

`/orchestrate` may exist in this repo as a convenience command that preloads
Herdr environment, current pane, branch and working-tree state. It is routed
as a Herdr helper, not as a peer of `/plan` / `/implement` / `/learn`.

## Alternatives Considered

| Option | Why rejected |
|--------|-------------|
| ALT-001: couple orchestrate to the pinst harness (`.ash/orchestrations/`, harness check, plan-implement boundary, lifecycle peer of plan-write) | The skill must run in any Herdr session. Tying it to the plan corpus makes non-plan repos and one-off delegated tasks second-class, and duplicates ownership that plan-implement already has for plan work |
| ALT-002: expose only the skill and rely on automatic routing | Callers still benefit from preloaded `HERDR_ENV`, pane, branch and dirty state when this repo ships a command. A thin `/orchestrate` is fine as packaging; it must not redefine the skill as a harness stage |
| ALT-003: use one topology for every delegated task | Always creating a worktree adds branch/workspace overhead to read-only investigation; always sharing the current checkout permits concurrent writers and cannot isolate a reviewable result. Selecting by mutation and isolation needs preserves both safety and the lightweight sibling-pane path |
| ALT-004: put orchestration logs under `.ash/` "for consistency" | `.ash/` is the plan corpus. Putting non-plan run logs there implies harness ownership and checkability that CON-004 forbids |

## Consequences

Delegation becomes reproducible via a portable log any tool can read, without
requiring pinst or a plan directory. The distinction between Herdr's UI
lifecycle and semantic task completion becomes explicit. This repo can still
wire the skill for convenience, but other checkouts and machines get the same
procedure from the skill text alone.

The cost is no mechanical corpus check for orphaned orchestration logs
(accepted under CON-004 / GUD-001). Prompt retention improves diagnosis but
creates a redaction responsibility. Preserving worktrees after terminal
outcomes avoids destructive cleanup but leaves housekeeping to an explicit
later action.

- **DEP-001:** Herdr's installed agent-facing contract and socket API. Version
  0.9.1 was verified during planning with `herdr --skill`, command help and
  live read-only JSON calls; implementation rechecks those commands before
  encoding examples. Cursor's optional integration currently reports
  `not installed`, which is an observed preflight state rather than an
  assumption that implementation may ignore.
- **DEP-002:** Cursor Agent's native `--model` option, passed through Herdr
  only after `--`. Other Herdr kinds use their own native arguments and are
  not forced into Cursor's syntax.
- **ASSUMPTION-001:** A default log directory under `.herdr/orchestrate/`
  (cwd- or worktree-relative) is acceptable everywhere Herdr is used; if a
  host forbids dotdirs, the skill documents an override without falling back
  to `.ash/`.
- **RISK-001:** Herdr's `idle` and `done` both mean ready for input, while
  `done` can differ by client seen-state. Mitigation: neither is sufficient
  for `resolved`; the supervisor reads output and verifies acceptance evidence.
- **RISK-002:** A timeout or `agent_prompt_stalled` does not prove the prompt
  was not delivered. Mitigation: `get` and `read` precede every retry, and a
  nudge names the observed missing condition rather than replaying the task.
- **RISK-003:** A new worktree starts from a committed ref and excludes dirty
  parent state. Mitigation: preflight checks `git status`; dependency on dirty
  content is escalated before creation, with no automatic stash, copy or
  commit.
- **RISK-004:** Durable prompt text can contain secrets. Mitigation: the skill
  requires redaction before `prompt_sent` and permits a summary plus omitted
  field names when exact text is unsafe.
- **RISK-005:** Without a harness checker, a crashed run's log may sit forever
  without `run_end`. Mitigation: accept that; the skill still writes `run_end`
  on every supervised exit path, and operators may delete stale files. Do not
  invent calendar-based "stale" findings in pinst (LESSON-011).
- **RISK-006:** Automatically removing a delegated worktree can destroy the
  only copy of an uncommitted result. Mitigation: cleanup is outside the
  orchestration terminal path and requires separate authorization.
- **RISK-007:** A supported agent executable can be present while its Herdr
  lifecycle integration is absent or outdated, weakening state classification.
  Mitigation: log integration status, never install into a user's home
  implicitly, verify actual transitions during startup, and escalate persistent
  `unknown` or unreliable blocker detection instead of claiming unattended
  supervision.

## Phases

| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Portable skill contract and log format | [phase-01.md](phase-01.md) | Done |
| 2 | Herdr topology, handoff and babysitting | [phase-02.md](phase-02.md) | Done |

## Affected Files

- **FILE-001:** `.agents/skills/orchestrate/SKILL.md` (new) — harness-agnostic
  trigger, Herdr preflight, topology decision, handoff, supervision state
  machine, resolution test, recovery policy, portable logging contract.
- **FILE-002:** `.agents/commands/orchestrate.md` (new) — optional convenience
  entry point with Herdr/pane/git context; explicitly not a plan-lifecycle
  stage.
- **FILE-003:** `.claude/skills/orchestrate` (new symlink) — runtime projection
  created by `just wire`.
- **FILE-004:** `.claude/commands/orchestrate.md` (new symlink) — runtime
  command projection created by `just wire`.
- **FILE-005:** `.agents/README.md` — document `/orchestrate` as a Herdr helper
  outside the plan-write → implement → learnings chain; do not add it to the
  harness lifecycle diagram as a peer stage.
- **FILE-006:** `docs/tools/herdr.toml` — optional recipe/see_also pointer to
  the orchestrate skill for discovery via `pinst docs`, without making docs
  a runtime dependency of the skill.

## Open Questions

None. Harness-agnosticism supersedes the earlier draft's choice to place logs
under `.ash/` and grade them with `pinst harness check`.
