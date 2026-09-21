---
name: orchestrate
description: 'Delegate a task to another coding agent through Herdr, choosing a sibling pane or isolated worktree, and supervise it until the task is verified as resolved. Use when the user explicitly asks to orchestrate, delegate, babysit, or run work in another Herdr pane or worktree.'
produces: none
kind: reference
---

# Orchestrate

Coordinate another coding agent through Herdr and remain responsible for the
delegated task until its result is verified, escalated, or explicitly aborted.
Herdr's terminal state is an observation; it is never, by itself, proof that
the task is complete.

This skill is portable and harness-agnostic. It requires Herdr, not pinst:

- It does not require `.ash/`, a plan id, or any plan lifecycle skill.
- It does not edit plan checkboxes, statuses, or changelogs.
- It does not invoke `pinst harness` or register repository findings.
- It works outside git repositories when the requested topology does not need
  a git worktree.

## Artifacts

Every orchestration writes one append-only JSON Lines log. By default:

```text
.herdr/orchestrate/<YYYYMMDDTHHMMSSZ>-<agent-name>.log
```

Resolve that path relative to the orchestrator's starting working directory.
When the caller supplies a log directory, use it instead; never silently fall
back to `.ash/`. Create the directory before the first event. Keep the log
after both success and failure unless the user separately asks to remove it.

The filename start time and every event timestamp are UTC. Write one compact
JSON object per line and append only; never rewrite earlier events to make a
run look cleaner than it was.

Each event has this envelope:

```json
{"ts":"2026-09-21T20:15:00Z","event":"run_start","run":"20260921T201500Z-reviewer","workspace":null,"pane":null,"agent":null,"worktree":null,"message":"starting delegated review","detail":{}}
```

Fields:

- `ts`: RFC 3339 UTC timestamp.
- `event`: one value from the vocabulary below.
- `run`: stable `<start-time>-<agent-name>` identifier matching the filename.
- `workspace`, `pane`, `agent`, `worktree`: known Herdr ids, agent name, and
  checkout path; use JSON `null` until each exists.
- `message`: concise human-readable summary.
- `detail`: object containing structured evidence such as the selected
  topology, sanitized prompt, observed state, timeout, verification command,
  result, or abort reason.

Event vocabulary, in lifecycle order:

| Event | Write it when |
|-------|---------------|
| `run_start` | The orchestration begins; include the task summary and starting cwd |
| `pane_created` | A sibling pane is created |
| `worktree_created` | A worktree workspace is created or opened |
| `agent_started` | Herdr detects the selected coding agent as ready |
| `prompt_sent` | A sanitized initial prompt or targeted follow-up is submitted |
| `wait_result` | A wait settles, stalls, times out, or errors |
| `blocked` | Herdr or the transcript shows an approval/question blocker |
| `nudge` | One targeted follow-up addresses a concrete missing condition |
| `escalated` | The user must decide, authorize, or supply missing information |
| `resolved` | The response, artifact, checks, and blocker state were verified |
| `aborted` | The run cannot continue safely; include the concrete reason |
| `run_end` | Final event; `detail.status` is `resolved` or `aborted` |

`resolved` must include the acceptance evidence in `detail.verification`.
`aborted` must include `detail.reason`. Exactly one of those terminal events
precedes `run_end`, whose status must agree. A live run may end temporarily at
any earlier event if the orchestrator process is interrupted; do not invent a
terminal outcome while recovering it.

Prompts are useful audit evidence, but logs are durable. Before writing
`prompt_sent`, remove credentials, tokens, private keys, cookies, and other
secrets. If redaction would make the exact text misleading, store a concise
summary and list the omitted field names in `detail.redacted`.

## Non-goals

This is not a plan lifecycle stage, a generic parallel implementation
framework, or an automatic merge/cleanup service. It does not decide how a
delegated repository records its own work. It does not treat an idle terminal
as success, approve interactive prompts for the user, install Herdr
integrations, trust repositories, or remove panes/worktrees without explicit
authorization.

The Herdr preflight, topology choice, handoff, supervision loop, and recovery
procedure follow in the next section of this skill.
