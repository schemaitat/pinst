---
name: orchestrate
description: 'Delegate and supervise a bounded task through Herdr until it is verified as resolved, including feature implementation requests that must run the implement-feature delivery chain. Use when the user explicitly asks to orchestrate, delegate, babysit, or run work in another Herdr pane or worktree.'
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

When the delegated request is to implement, build, or ship a feature end to
end, recognize it as an `implement-feature` task. The child must run the full
feature chain in its isolated worktree: persist a plan with `plan-write`,
execute it with `plan-implement` (including `plan-learnings` on every
outcome), verify the repository quality gate and acceptance criteria, and use
`create-pr` when publication is authorized. The orchestrator supervises that
whole chain through semantic resolution; it must not stop merely because the
child delegated the work to another skill, became idle, or created a branch.
The initial prompt must name the exact feature acceptance criteria and the
required terminal evidence so the child cannot treat delegation as completion.

## Hard boundary — delegate, never implement

**The orchestrator only delegates. It must not do the delegated task itself.**
The child agent owns the plan, feature, fix, investigation, or other
deliverable named in the handoff. This orchestrator session is limited to:

- creating and inspecting Herdr topology;
- starting, prompting, waiting for, reading, and inspecting the child;
- sending keys for a blocker only after explicit user authorization;
- verifying acceptance by reading output/artifacts and running checks;
- writing the portable orchestration log;
- one targeted nudge per missing condition, escalation, or abort.

Even when it would be faster, the orchestrator must never write, edit, or
commit the child's deliverable; implement any portion of the handed-off task;
"help finish" because the child is slow or blocked; or enter the delegated
worktree as a second writer. Acceptance checks may observe the child's result,
but failures go back to the child in a bounded prompt. They are not repaired
in the orchestrator pane.

If the next action would implement the child's task, stop. Put that work into
a prompt to the child and supervise it. If the child cannot proceed, escalate
or abort instead of taking over.

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
| `command_error` | A Herdr control command fails; include status and structured error before inspecting live state |
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

It is also not a second implementer. The orchestrator never edits or commits
the child's deliverable, executes the delegated implementation itself, or
co-edits the delegated worktree. A slow, incomplete, or blocked child triggers
inspection, one targeted nudge, escalation, or abort — never parent-pane
takeover.

## Step 1 — Prove Herdr caller context

The user must explicitly ask for Herdr orchestration or delegation. Before any
Herdr inspection or control command:

```bash
test "${HERDR_ENV:-}" = 1
```

If this fails, say that this session is not running inside Herdr and stop. Do
not inspect or control whichever Herdr session happens to be focused outside
the caller.

When it passes, refresh the installed contract rather than trusting examples
in this file:

```bash
herdr --skill
herdr --help
herdr agent
herdr worktree
herdr pane
herdr integration status
```

Never run bare `herdr`; it launches or attaches the TUI. Run mutating nested
commands only after checking their `--help`, because some create commands have
valid defaults and execute when arguments are omitted.

Record `run_start` before the first mutation. Include the user's task, starting
cwd, requested kind/model, acceptance criteria, and the caller's
`HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, and `HERDR_PANE_ID` when present.

Herdr socket control commands return JSON on stdout. Server/API errors are
JSON on stderr with exit 1; syntax errors exit 2. Local metadata commands such
as `integration status` may render text, so use them only for their documented
local purpose. For socket control, branch on status and fields, not on human
terminal rendering. Public ids are opaque and scoped to one server: preserve
values from responses instead of deriving them from labels, examples, or
sidebar order.

## Step 2 — Bound the delegated task

Before creating layout, write down:

- one concrete goal;
- whether the child may edit files;
- the exact cwd or repository and committed base it should see;
- allowed and forbidden mutations;
- acceptance criteria the orchestrator can independently inspect;
- the requested agent kind and optional model;
- conditions that require user escalation;
- the final report shape.

For an `implement-feature` handoff, also require the child to report the
isolated worktree and branch, committed base, persisted plan id/path, phase
and task outcome, run-log and learnings paths, quality-gate result, PR URL
when authorized, and every unresolved blocker. Publication, credentials,
trust, dirty-base choices, destructive cleanup, and other user decisions
remain approval points; the orchestrator escalates them rather than answering
for the user.

Do not delegate an open-ended request such as "finish everything". If
acceptance cannot be observed from the response, filesystem, git state, or a
command, tighten the task before starting an agent.

If a git checkout is present, inspect `git status --short`, the branch, and
`git rev-parse HEAD`. A new worktree starts from a committed ref. If the task
depends on uncommitted parent content, stop and ask the user to choose a safe
handoff; never stash, commit, copy, or omit those changes silently.

## Step 3 — Choose exactly one topology

Use a sibling pane when all of these are true:

- the task is read-only, a test run, or another non-mutating operation;
- sharing the current cwd cannot race another writer;
- the result is a response or report rather than an isolated branch.

Inspect the caller geometry:

```bash
herdr pane layout --pane "$HERDR_PANE_ID"
```

Split a wide pane right and a narrow/tall pane down, preserving cwd and focus:

```bash
herdr pane split --current --direction right --cwd "$PWD" --no-focus
```

Replace `right` with `down` when appropriate. Parse the new pane from
`.result.pane`; append `pane_created` with its actual ids and cwd.

Use a worktree workspace when the child may edit, should produce a reviewable
branch, or must be isolated from the caller. Require a git checkout and a
committed base. First discover the repository source workspace:

```bash
herdr worktree list --cwd "$PWD"
```

When the caller is already in a linked worktree, create/open actions must
target `.result.source.source_workspace_id` and use
`.result.source.source_checkout_path`; Herdr rejects a linked worktree as the
source. Preserve those JSON values rather than guessing that the primary
workspace is focused. Then inspect current syntax:

```bash
herdr worktree create --help
herdr worktree create \
  --workspace "<source-workspace-id>" \
  --branch "<unique-branch>" \
  --base "<committed-ref>" \
  --label "<short-label>" \
  --no-focus
```

`--workspace` and `--cwd` are alternative source selectors; never pass both.
Use the returned source workspace when one is open, otherwise use the source
checkout path with `--cwd`.

Never add `--trust-repository` unless the user has explicitly verified and
authorized that repository. Parse the returned workspace and root pane from
the JSON response, then use those returned values for every later command.
Append `worktree_created` with the branch, base, checkout path, workspace id,
and pane id.

Do not create a worktree merely because one is available. Do not put two
writers in one checkout merely because a sibling pane is cheaper.

If a control command fails, append `command_error` with its exit status and
JSON error, then inspect current worktree/workspace/pane state before retrying.
A connection failure does not prove a mutation was not applied.

## Step 4 — Start and verify the child

The target pane must be an available interactive shell with no foreground
command. Pick a useful name matching `[a-z][a-z0-9_-]{0,31}` and confirm it is
unique with `herdr agent list`.

Check `herdr integration status` for the selected kind and record the result.
An absent or outdated optional integration is not permission to install it.
It is a warning that observed state may degrade to `unknown`; the supervision
loop must prove actual transitions before relying on them.

Start Cursor with:

```bash
herdr agent start <name> --kind cursor --pane <pane-id>
```

When the user requested a Cursor model, pass the installed Cursor CLI argument
after Herdr's `--` separator:

```bash
herdr agent start <name> --kind cursor --pane <pane-id> -- --model <model>
```

Do not guess a model when none was requested. For another kind, pass native
arguments only when the user supplied them or the installed help confirms
them.

Success means Herdr detected the expected agent in that pane and considers it
ready. If start returns `agent_not_ready`, keep the returned name/pane, inspect
`herdr agent get` and `herdr agent read`, and follow the blocked/unknown rules
below. Append `agent_started` only after verifying the reported agent, pane,
cwd, and readiness match the handoff. A cwd mismatch is an escalation, not a
prompting opportunity.

## Step 5 — Submit one complete initial prompt

The initial prompt must stand alone. Include:

- the bounded goal and acceptance criteria;
- the exact checkout/cwd and branch/base when applicable;
- allowed mutations and safety constraints;
- commands or checks that must pass;
- the required final response: summary, changed artifacts, verification,
  unresolved blockers.

Submit through the agent surface:

```bash
herdr agent prompt <name> "<bounded prompt>" --wait --timeout 120000
```

The timeout is a caller policy, not a universal constant; choose one suitable
for the task. Without `--until`, Herdr waits for the first settled
`idle`, `done`, or `blocked` state after observed activity. This does not track
the task or even a particular turn.

Prepare the sanitized prompt for logging, but append `prompt_sent` only after
the command confirms submission or `get`/`read` proves the prompt reached the
child. Always append `wait_result` with the command outcome, observed status,
and timeout. Submission success proves only that input and Enter were written.
`agent_prompt_stalled` or `timeout` does not prove the prompt was absent.
After inspection, resend the exact initial prompt at most once only when the
transcript and state prove it never appeared and no work began; log that
evidence. Otherwise continue waiting or send a targeted nudge rather than
duplicating the task.

## Step 6 — Babysit to semantic resolution

After every prompt or wait, inspect both state and output:

```bash
herdr agent get <name>
herdr agent read <name> --source recent-unwrapped --lines 120
```

Increase `--lines` when needed. If terminal history cannot recover a long
answer, ask the child as a targeted follow-up to write a Markdown report in a
temporary directory and reply only with its path; then read that file. Do not
use file output as the initial handoff.

Interpret states this way:

| State/outcome | Required action |
|---------------|-----------------|
| `working` | Continue waiting; do not prompt over active work |
| `idle` | Read output and verify acceptance; ready for input is not resolved |
| `done` | Read output and verify acceptance; seen-state is not resolved |
| `blocked` / `agent_blocked` | Read the approval/question UI, append `blocked`, and escalate to the user without answering it |
| `unknown` | Read output, perform one bounded diagnostic wait, then escalate if classification remains unreliable |
| `agent_prompt_stalled` | Inspect get/read before deciding whether any input is needed |
| `timeout` | Inspect get/read; if still working, wait again; if the prompt is provably absent, submit it once; otherwise evaluate observed output without replay |
| missing/exited agent | Inspect pane output and escalate or abort with the concrete evidence |

Resolution requires all of:

1. the child explicitly reports what it did and any remaining blockers;
2. the response addresses the full delegated goal;
3. every promised file, diff, branch, command result, or report exists;
4. acceptance checks pass when independently inspected;
5. no approval, question, or unresolved condition remains.

For an `implement-feature` task, “every promised artifact” additionally
means the plan is persisted under the repository's plan corpus, all required
phases are complete or the user explicitly chose a partial draft outcome,
`plan-learnings` recorded the implementation result, the quality gate passed,
and `create-pr` produced the requested PR. A partial or local-only result is
resolved only when that scope was explicitly authorized; otherwise it is
blocked or aborted, never silently downgraded.

Only then append `resolved` with `detail.verification`.

If a settled response is incomplete but the missing condition is concrete,
send one targeted follow-up for that condition and append `nudge`. Never replay
the initial prompt. Track nudges by condition: separate missing requirements
may receive separate nudges, but the same condition recurring after one nudge
must be escalated.

Do not repair a failed acceptance check yourself. Report the exact observed
failure to the child as its targeted nudge; if it persists, escalate or abort.

Append `escalated` and ask the user when:

- approval, credentials, trust, integration installation, or a product choice
  is required;
- dirty parent state prevents a safe worktree handoff;
- a blocker or `unknown` state cannot be classified reliably;
- the requested result conflicts with observed artifacts;
- the same missing condition survives its targeted nudge.

If the user declines, the child exits without a usable result, or progress
cannot continue safely, append `aborted` with the reason and then `run_end`
with `detail.status: "aborted"`. Do not describe an unresolved task as done.

## Step 7 — Finish without destructive cleanup

For a verified result, append `resolved`, then `run_end` with
`detail.status: "resolved"`. Report:

- outcome and verification;
- child name, kind/model, workspace and pane;
- worktree path/branch when applicable;
- orchestration log path;
- any resource deliberately left running.

Do not automatically close panes/workspaces, remove worktrees, install
integrations, merge branches, or discard child changes. Those are separate
mutations requiring explicit user authorization. On a failed remote or socket
operation, inspect live state before retrying because the mutation may already
have been applied.
