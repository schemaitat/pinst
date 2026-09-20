---
name: plan-implement
description: Autonomously implement a plan written by the plan-write skill, working phase by phase through its ADR docs under .ash/plans/<id>-<slug>/, ticking off tasks, and keeping a structured run log under .ash/plans/<id>-<slug>/logs/. Use whenever the user asks to implement, execute, start, resume, or continue a plan (e.g. "implement 260919-qwerty", "start on the mlflow plan", "continue implementing this", "work through phase 2"), even if they don't name the skill directly.
produces: 'every plan that has been worked on carries a run log with a matching run_start and run_end'
evidence: 'echo $(for d in .ash/plans/*/; do grep -qh ''"event":"run_start"'' $d/logs/*.log 2>/dev/null && grep -qh ''"event":"run_end"'' $d/logs/*.log 2>/dev/null && echo x; done | grep -c x) $(ls -d .ash/plans/*/ | grep -c .)'
---

# Plan Implement Skill

Implements a plan written by `plan-write`: works through its phases and tasks
in order, ticks off checkboxes and flips `status` frontmatter as work lands, and
appends a structured, append-only log of what happened to
`.ash/plans/<id>-<slug>/logs/`. The plan doc is the source of truth for *what*
to build; the log is the audit trail of *what actually happened* when it was
attempted — especially failures and surprises, which are the first things
lost once a session ends and the easiest thing for a future run (or a human)
to need.

Standard git safety rules still apply here (stage specific files, no
force-push, no skipped hooks, confirm before destructive ops) — the
"autonomous" part of this skill is about not stopping to ask permission
between routine phases of already-approved work, not about relaxing those
rules.

______________________________________________________________________

## Step 1 — Identify the plan

- If the user names an id or slug, use it directly:
  `.ash/plans/<id>-<slug>/`.
- Otherwise, read `.ash/INDEX.md` (or list `.ash/plans/`). If exactly one
  plan has frontmatter `status: In Progress`, resume that one. If there's more than one candidate (several
  in progress, or none and several `Proposed`), ask the user which plan
  they mean (AskUserQuestion) — guessing which unfinished plan to pick up
  is not a call to make silently.
- Read `README.md`, and every `phase-NN.md` if the plan is multi-phase.

## Step 2 — Determine where to resume

- **Single-phase plan**: scan the `## Steps` checklist in `README.md`. The
  first unchecked `TASK-NNN` is where work resumes.
- **Multi-phase plan**: each `phase-NN.md`'s frontmatter `status` is
  authoritative (the README's `## Phases` table mirrors it). The first
  phase not marked `Done` is current; within its `phase-NN.md`, the first
  unchecked `TASK-NNN` is where work resumes.
- Respect the plan's own linear chain (this is why `plan-write` enforces
  it): never start a phase or task before its immediate predecessor is
  Done, and never touch a phase already marked Done.
- If every phase and task is already Done, tell the user there's nothing
  left to implement and stop here.

## Step 3 — Start the run log

- Directory: `.ash/plans/<id>-<slug>/logs/` — create it if it doesn't
  exist. Logs live inside the plan's own folder so one directory holds
  everything about one unit of work: the decision record, the runs that
  implemented it, and what was learned.
- Filename: `<start-time>-<model-id>.log`, where `start-time` is this run's
  start time as `YYYYMMDDTHHMMSSZ` (UTC, sortable) and `model-id` is this
  session's model identifier (e.g. `claude-sonnet-5`). A plan is often
  implemented across several separate runs/sessions — the filename is what
  keeps those runs distinguishable at a glance.
- Format: **JSON Lines** — one compact JSON object per line. This keeps the
  log greppable and machine-parseable, and means a run that dies mid-way
  never corrupts the lines already written.
- **Append only.** Never truncate or rewrite past lines, including on
  resume — the log's value is the full history across every run for this
  plan, not just the current one.

Each line follows this shape:

```json
{"ts": "2026-09-19T10:42:03Z", "event": "task_done", "plan": "260919-qwerty-add-mlflow-experiment-tracking", "phase": 1, "task": "TASK-003", "message": "wired MLflow client into the training loop", "detail": {}}
```

`phase` and `task` are `null` when not applicable (e.g. `run_start`).
`detail` is an object for anything structured worth keeping (a failing
command, an error message, a commit hash) — omit it or leave it `{}` when
`message` already says everything.

Event vocabulary:

| event | when |
|-------|------|
| `run_start` | first line of every run — include the resume point and `git rev-parse HEAD` in `detail` |
| `phase_start` | beginning a phase |
| `task_done` | a task's change is made and working |
| `task_failed` | a task could not be completed — `detail` holds the concrete reason |
| `phase_done` | every task in a phase is done and its Done criteria pass |
| `phase_failed` | tasks done but Done criteria failed — `detail` holds what failed |
| `issue` | something short of outright failure: a wrong assumption, an ambiguous requirement, a flaky test, drift between the plan and the current codebase |
| `commit` | after committing this plan's work — `detail.hash` and `detail.subject` |
| `run_end` | last line of every run — `detail.status` is `completed`, `aborted`, or `blocked` |

## Step 4 — Implement phase by phase

Starting from the resume point, for each phase in the plan's own order:

1. Log `phase_start`. Set that phase's frontmatter `status` to
   `In Progress` (in `phase-NN.md` if multi-phase, else `README.md`), and
   mirror it into the README's `## Phases` table in the same edit — the
   plan docs and the log should never disagree about where things stand.
   Bump the README's frontmatter `updated` date whenever you touch it.
2. Work through its `TASK-NNN` items in chain order. After each one:
   - **Succeeded**: tick its checkbox in the `.md` file, log `task_done`
     with a one-line summary (the git history already has the diff, so
     the log doesn't need to repeat it).
   - **Failed**: log `task_failed` with the concrete reason in `detail`,
     leave the checkbox unchecked, log `run_end` with `detail.status:
     "aborted"`, and stop. The chain means every later task depends on
     this one — continuing would build on a broken foundation.
3. Once every task in the phase is checked, verify the phase's `## Done
   criteria` (the `TEST-NNN` items). If they pass: set frontmatter
   `status: Done` (and its mirror in the `## Phases` table), log
   `phase_done`. If they fail: log `phase_failed` with `detail`, log
   `run_end` aborted, and stop.

   When the criteria are separable — the CLI contract, the JSON envelope, the
   exit codes — verifying them is read-only and independent, so it can be
   delegated in parallel. Implementing is not: the phase chain is a
   correctness property, and concurrent writers race on the checkboxes,
   the frontmatter, `INDEX.md`, and the append-only changelog. See
   "Delegating" in `.agents/README.md`.
4. Commit the phase's changes (use the `conventional-commits` skill),
   including a `Plan: <id>-<slug>` footer so the commit traces back to
   this plan. One commit per phase keeps history traceable phase-by-phase
   unless the plan calls for finer-grained commits per task. Log the
   result as a `commit` event with the hash and subject.
5. Append an entry to `.ash/CHANGELOG.log` for this phase (see "Updating
   the changelog" below).
6. Continue to the next phase.

If every phase completes, set the README's frontmatter `status: Done`, log
`run_end` with `detail.status: "completed"`, and append one more
`.ash/CHANGELOG.log` entry marking the whole plan done.

Whenever a `status` changes — at any point in a run, not just at the end —
regenerate the index and re-check the corpus:

```sh
just index          # rewrite .ash/INDEX.md from the plan frontmatter
just harness        # 0 clean, 3 findings to act on
```

The index is only worth consulting if it's never stale, and a run that aborts
halfway is exactly when an accurate "what's in progress" row matters most.
`just harness` also catches the failure mode this step invites: flipping a
phase's frontmatter `status` but not its mirror in the README's `## Phases`
table, which it reports as a `phase.status-mirror.*` finding.

### Updating the changelog

`.ash/CHANGELOG.log` is a single append-only file shared by every plan —
where the per-run log is a detailed, per-plan record for debugging one
run, the changelog is the project-wide, at-a-glance history of what has
actually shipped. Keep entries to one line each, in
[logfmt](https://brandur.org/logfmt) style, so the file stays skimmable
with `tail` and greppable by `id=` or `slug=` without a JSON parser:

```
ts=2026-09-19T10:42:03Z id=260919-qwerty slug=add-mlflow-experiment-tracking phase=1 summary="wired MLflow client into the training loop" refs=".ash/plans/260919-qwerty-add-mlflow-experiment-tracking/phase-01.md,.ash/plans/260919-qwerty-add-mlflow-experiment-tracking/logs/20260919T104203Z-claude-opus-5.log"
```

Fields:

- `ts` — UTC timestamp of the entry, same format as the run log.
- `id` / `slug` — the plan's identifier, split so either half greps
  cleanly on its own.
- `phase` — the phase number just completed, or omitted for the
  whole-plan-done entry.
- `summary` — one clause of what was actually delivered, in plain
  language a human skimming the file would want (not a task ID dump).
- `refs` — comma-separated paths a reader would open next: at minimum the
  plan/phase file and this run's log file; add specific changed files
  only when they add something the summary didn't already say.

Quote any field whose value contains a space, and never let a plan's own
content (a phase title, a task description) get interpreted as a second
logfmt field — treat it as the value of whichever field it belongs in.

## Step 5 — Issues that aren't outright pass/fail

Not everything is a clean success or failure. If an `ASSUMPTION-NNN` the
plan made turns out false, a requirement is ambiguous once you're actually
implementing it, or the codebase has drifted since the plan was written,
log an `issue` event with what you found. Use judgment about whether it's
minor enough to note and keep going, or significant enough to stop and ask
the user (AskUserQuestion): the plan exists so the *what* and *why* were
already decided, so silently improvising past a wrong assumption defeats
the point of having one. Prefer stopping over guessing whenever the
deviation would change what the plan actually delivers.

Never edit a plan's `Context`, `Decision`, or `Alternatives Considered`
sections while implementing — only `status` fields and task checkboxes
change during implementation. If the approach itself needs to change,
that's a plan revision (back to `plan-write`), not something this skill
does on its own.

## Step 6 — Hand off to plan-learnings

Once the run ends — whatever the outcome, `completed`, `aborted`, or
`blocked` — invoke the `plan-learnings` skill for this plan before reporting
back. Implementation isn't finished until whatever went wrong (or went
smoothly) is captured at `.ash/plans/<id>-<slug>/learnings.md`; skipping this on
a "successful" run is exactly how the same mistake gets repeated silently
next time. `plan-learnings` will use this session's own context as its source,
since it's being invoked right after the work happened.

## Step 7 — Report back

At the end of a run summarize for the user: which phases/tasks were
finished this run, what failed or is still open, the plan's current
`status`, and the paths to the run log and the (now updated) learn file.
Point to those files rather than pasting their contents; they're meant to
be read with `grep`/`jq` when something needs investigating, not
reproduced inline.
