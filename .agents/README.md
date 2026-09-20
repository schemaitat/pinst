# The agent harness

The `pinst` skill is the contract for *driving pinst*. This file is the
contract for *working on pinst* — how an agent plans a change, implements it,
and writes down what it learned, and what keeps those three things from
drifting apart. (`AGENTS.md` at the repo root is deliberately one line: it is
always in context, so it carries only what is always true.)

Two directories hold the whole thing:

```
.agents/skills/     the skills — vendor-neutral source of truth
.ash/               the corpus — every plan, log, and lesson this repo has produced
```

A plan is identified by `<yymmdd>-<six letters>`, and lives in
`.ash/plans/<id>-<slug>/`:

```
.ash/plans/260919-qwerty-add-mlflow-experiment-tracking/
           └─── id ────┘ └─────────── slug ───────────┘
```

Both halves earn their place. The random half needs no coordination, which
is the point: work happens in parallel git worktrees branched from the same
commit, so any "next number in sequence" scheme has two sessions computing
the same answer and colliding at merge — by which time the identifier is
already written into commit footers and can no longer be changed. The date
half is `yymmdd`, most-significant-first, so string order is date order and
`ls .ash/plans/` comes out chronological with no sort key.

Mint one with `scripts/ash.sh new-id`; never derive one by counting.

Everything else named here is projection or enforcement.

## The lifecycle

One unit of work runs top to bottom. Each arrow is a handoff with a defined
input and a defined artifact, not a suggestion.

```
 ┌────────────┐
 │ plan-write │──▶ README.md + phase-NN.md — the decision record
 └─────┬──────┘
       │ a plan id
       ▼
 ┌────────────────┐
 │ plan-implement │──▶ ticked checkboxes, logs/*.jsonl, CHANGELOG.log,
 └─────┬──────────┘    and one commit per phase via conventional-commits
       │ a run outcome
       ▼
 ┌────────────────┐
 │ plan-learnings │──▶ learnings.md, and what generalizes → LEARNINGS.md
 └─────┬──────────┘
       │
       └──▶ read by the next plan-write, before it picks an approach
```

| Skill | Reads | Writes | Ends by |
|-------|-------|--------|---------|
| `plan-write` | the session, `.ash/LEARNINGS.md` | `.ash/plans/<id>-<slug>/` | handing back a plan id |
| `plan-implement` | that plan, phase by phase | checkboxes, `logs/`, `.ash/CHANGELOG.log` | invoking `plan-learnings` |
| `plan-learnings` | session context, or `logs/` | `learnings.md`, `.ash/LEARNINGS.md` | reporting both paths |
| `conventional-commits` | the diff, `git log` | a commit | — |

`conventional-commits` is not a lifecycle stage; it is called *by*
`plan-implement` once per phase, and directly by a human via `/cc`.
`create-pr` sits alongside it, at the other end: it reads the same plan the
lifecycle produced and turns it into the PR description, so the reasoning
recorded at planning time is what reviewers actually get.

Neither is `pinst`, which is reference rather than procedure: the contract for
driving the CLI this repo builds — its JSON envelope, exit codes, step
outcomes and doctor findings, and how to add a tool to `manifest.toml`. It
lives as a skill so it loads when an agent is actually about to run `pinst`,
instead of sitting in every session's context. That is why `AGENTS.md` is now
one line.

The loop closes at `plan-learnings` → `.ash/LEARNINGS.md` → the next
`plan-write`, which reads it before choosing an approach. That file is the
only reason the corpus is worth keeping rather than just being history.

The corpus-wide pass that closes it — `plan-learnings` Step 6, reached by
`/distil` — has a second entry point that needs no human:
`.github/workflows/distil.yml` runs it daily and opens a pull request with
whatever it decided. The schedule only polls; `scripts/distil-guard.sh
preflight` asks the corpus whether there is anything to distil, and a clean
one ends the run before a model is ever started. That is LESSON-013 surviving
a cron line — the corpus is still the clock.

### Two fields make the loop measurable

Both are written by `plan-learnings` and read by `scripts/ash.sh`:

- **`**Status:**` on every lesson** in `.ash/LEARNINGS.md` — `prose`,
  `mechanized` or `retired`. A `mechanized` lesson also carries
  **`**Check:**`**, naming the finding id that now enforces it
  (`phase.logged-not-done`, `wire.missing` — an id, not a description). That
  id must exist somewhere under `scripts/`, in *either* checker: `ash.sh`
  owns the corpus invariants and `agents-wire.sh` owns the skill projection,
  and a lesson may be mechanized by either. `ash.sh check` reports
  `lesson.unenforced` when the id is nowhere to be found, because a lesson
  claiming enforcement it does not have is worse than one honestly marked
  `prose` — it tells the next reader the problem is handled.
- **`**Skill:**` on every issue** in a plan's `learnings.md` — the skill whose
  *instructions* would have had to change to prevent it, or `none`. Read by
  skill it is the failure tally in `ash.sh skills`; read by the plan's `areas`
  it is the missing-skill candidate list.

`Status:` exists so the file has an end state instead of only growing, and so
one question becomes answerable: how much of what we learned is actually
enforced? Today that is 2 of 10. The other eight are not a backlog — most
lessons are judgement that no exit code can carry — but a lesson sitting at
`prose` across several plans is a candidate for a check rather than for louder
prose.

## Routing

| The request | Entry point |
|-------------|-------------|
| "plan this", "write it up", "how should we do X" | `/plan` → `plan-write` |
| "implement plan 3", "continue", "do phase 2" | `/implement` → `plan-implement` |
| "what did we learn", "post-mortem this" | `/learn` → `plan-learnings` |
| "distil the corpus", "triage the untriaged" | `/distil` → `plan-learnings` Step 6 |
| "commit this" | `/cc` → `conventional-commits` |
| "open a PR", "ship this branch", "get this reviewed" | `/pr` → `create-pr` |
| "install X", "is this machine set up", "add a tool to the manifest" | `pinst` |
| a one-line fix with no design content | none of the above — just do it |

That last row matters. A plan is overhead that buys traceability; work that
nobody will need to trace next quarter should not pay for it. Reach for
`plan-write` when the *why* is worth more than the diff, which is roughly:
more than one phase, more than one plausible approach, or a decision someone
will later ask about.

## Slash commands are the entry points

`.claude/commands/` holds one command per lifecycle stage. They exist rather
than relying on the model to pick the right skill unprompted, and each one
front-loads the context that stage always needs — the plan corpus for
`/plan`, the in-progress plans for `/implement`, the diff and scope
vocabulary for `/cc` — so the skill starts with its inputs already in hand
instead of spending its first three tool calls collecting them.

## How the skills reach a runtime

**No agent runtime reads `.agents/skills/.`** Claude Code reads
`.claude/skills/`. So the skills are *projected*:

```sh
just wire        # .claude/skills/<name> -> ../../.agents/skills/<name>
```

The links are relative and committed, so a fresh clone or a new worktree
arrives already wired; `just wire` is only needed after adding, renaming, or
deleting a skill. `.agents/` stays canonical because it is the cross-vendor
convention — adding a second runtime means one more entry in
`TARGET_DIRS` in `scripts/agents-wire.sh`, not a second copy of every skill.

This mirrors what pinst does with `configs/`: one source tree, symlinked
into the place the consumer looks, so edits round-trip with no sync step.

If a runtime turns out not to follow symlinked skill directories,
`scripts/agents-wire.sh --copy` writes real copies instead, and
`--check` still verifies them — the projection is the contract, the link
style is an implementation detail.

## The checks

Both are in `just qc`, so the harness is held to the same standard as the
Rust. Both use pinst's own exit-code contract: `0` clean, `2` usage, `3` ran
fine and found things to act on.

```sh
just harness                      # both of the below
scripts/agents-wire.sh --check    # every skill wired, names match their dirs
scripts/ash.sh check              # the corpus invariants
scripts/ash.sh check --json       # ... as a machine-readable envelope
just index                        # regenerate .ash/INDEX.md
```

`ash.sh check` enforces what the skills previously only asserted in prose:
indices unique, quoted, and matching their directory; required frontmatter
present; `status` a legal value; no `## Status` section duplicating
frontmatter; phase files numbered `01..N` with no gaps; each phase's status
agreeing with the README's `## Phases` table; no plan marked `Done` over an
unfinished phase; a `learnings.md` wherever one is owed; and `INDEX.md`
matching the frontmatter it is generated from.

Three of those invariants exist to catch a corpus that has gone *stale*
rather than inconsistent — a plan that still advertises work the project
finished days ago:

- `plan.phases-all-done` — every phase is `Done` but the plan is not. The
  last phase closing is the moment a plan is over; leaving it open is how
  `INDEX.md` starts lying about what is in flight.
- `phase.logged-not-done` — `CHANGELOG.log` records a phase as shipped while
  the phase file still says `In Progress`. The log is append-only and written
  after the fact, so it is the half that cannot be wrong retroactively.
- `plan.unlogged` — a plan is `Done` with no `CHANGELOG.log` entry at all,
  which is the close-out skipped halfway.

They are the only checks in here that compare the corpus against a record of
what actually shipped, and they exist because plan `260919-zeuuaj` sat `In
Progress` for a day with its own changelog recording all four phases as done.

Two more keep the distillation loop honest, over the fields described under
"Two fields make the loop measurable" above:

- `learnings.untriaged` — a plan's `learnings.md` records an `ISSUE-NNN` that
  no lesson's `Seen in:` line references. This is the cadence: a plan closing
  starts the clock, and the clock stops when someone actually decides —
  promote it, add it to an existing lesson, or write `**Distilled:** declined
  — <reason>` on the issue, which silences it permanently. Three outcomes, one
  of them free, because a check that cries wolf is a check that gets deleted.
- `lesson.unenforced` — a lesson marked `**Status:** mechanized` whose
  `**Check:**` id no script under `scripts/` emits (or which names no id at
  all). An `error`, not a warning: a lesson claiming enforcement it does not
  have is worse than one honestly marked `prose`, because it tells the next
  reader the problem is handled.

Findings carry a stable `id` (`plan.id-mismatch.260919-qwerty-foo`) and a
`remediation` string, exactly like `pinst doctor` — match on the id, don't
parse the prose.

## Grading the skills

`just review` runs the invariants and then `scripts/ash.sh skills`, which
grades each skill **on the artifacts it leaves in the repo** — never on
whether anyone invoked it. Each skill declares its own contract in its
frontmatter:

```yaml
produces: 'every commit in this repo''s history parses as a Conventional Commit'
evidence: 'echo $(git log --format=%s $ASH_RANGE | grep -cE ...) $(git log ...)'
```

`evidence:` is a shell one-liner printing `<conforming> <total>`. It is run
twice — once with `ASH_WINDOW=20` and `ASH_RANGE=-n 20`, once with both empty
for all time — so a regression shows up while it is still one commit old
instead of being buried under a hundred conformant ones. A skill that produces
no artifact says `produces: none` with `kind: reference`; `pinst` is the only
one, and that exemption is written down rather than inferred from silence.

The measure lives in the skill because the skill is the only thing that knows
what it is for. Put it in the script and the two drift — which is the failure
this whole audit exists to catch. It is also code: `evidence:` is executed, so
review it like any other line in `qc`.

**A low rate is a conversation, not a failure.** The only findings here are
structural — `skill.no-contract` when a skill says nothing about what it
produces, `skill.evidence-failed` when the measure would not run. A measure
that cannot run reports `unmeasured` and never `0`, because a zero meaning
"offline" is worse than a gap that admits it.

Read the `issues` column against the rate. A skill at 100% conformance with
six issues naming it is producing perfectly-shaped artifacts by a procedure
that keeps going wrong, and it is the most interesting row in the table.

### Invocation counts are a second opinion

```sh
scripts/ash.sh skills --transcripts ~/.claude/projects
```

Off unless asked for, and nothing in `qc` may ever depend on it. With the flag
the report gains `fired` and `last seen`, counted from a runtime's own session
transcripts — **two** record shapes, because a skill has two front doors: a
`Skill` tool call carrying `input.skill`, and a slash command, which appears
as a `<command-name>` marker in user content. Counting only the first misses
`/cc` and `/pr` entirely.

These counts do not prove a skill was followed, and their absence does not
prove it was not. The evidence is blunt: `plan-implement` recorded **zero**
invocations of either shape before 2026-09-20 while having written a complete
run log — `run_start` through `run_end`, 18 `task_done` events — on the 19th.
Grading by invocation would have called the busiest lifecycle skill dead. So
they decorate the table; the artifact rates decide.

Everything in this path fails soft. An unreadable directory, a malformed line
or an unrecognised schema degrades to `unmeasured` and never changes the exit
code, because it reads an undocumented format owned by someone else's release
cycle. And it emits **derived counts only** — a skill name is printed only
after matching a directory in `.agents/skills`, so nothing typed into a
conversation can reach the output, and nothing here writes into `.ash/`.

## The review cadence

```sh
just review        # invariants, the per-skill report, the gaps, the agenda
/distil            # …and act on what it says
```

Run it when a plan closes, and whenever `qc` reports `learnings.untriaged`.
That is the whole schedule, and it is deliberate: **the corpus is the clock.**

A wall-clock cadence — a cron, a scheduled agent, a weekly reminder — fires
into silence on a quiet week and misses four plans on a busy one, and it lives
in one person's account rather than in the repo, so a fresh clone does not
inherit it. An `learnings.untriaged` finding fires exactly when there is
something to distil, stays until someone decides, and everyone who runs `qc`
sees it. If a clock is ever wanted anyway, it is one recipe — but it would be
a second trigger for something that already has one.

The agenda's last section is the part no exit code can settle: which recurring
work has no skill to do it. It is reported and never enforced, because a
missing skill is a judgement about what is worth automating and `qc` should
not fail over an opinion.

**Every recurring question can be told it has been answered.** A report that
asks something on every run, with no way to record the reply, decays into
noise at exactly the rate people read it. There are three markers, all the
same shape — one line, permanent, and a normal outcome rather than a failure
to think:

| Marker | Written on | Silences |
|--------|-----------|----------|
| `**Distilled:** declined — <reason>` | an issue | `learnings.untriaged` |
| `**Gap:** answered — <reason>` | an issue | the gap report |
| `**Mechanize:** declined — <reason>` | a lesson | "recurring, unenforced" |

They answer different questions about the same record — *has anyone distilled
this*, *should a skill own this*, *should a check enforce this* — so an issue
can carry two of them with different reasons. Most recorded issues are domain
surprises: a toolchain, an API, a permission, a GitHub object's lifecycle. No
instruction would have prevented them, `LEARNINGS.md` is already the right
home, and saying so once should be enough.

## Delegating

The plan skills are deliberately sequential: `plan-write` enforces a strictly
linear phase chain, and `plan-implement` refuses to start a phase before its
predecessor is `Done`. That constraint is about *the order work lands*, and
it says nothing about how many agents gather the information behind it.

Fan out, in parallel, for read-only work whose results merge cleanly:

- **Surveying the codebase** during `plan-write` Step 1 — "which modules
  touch the manifest schema", "where is version probing done" — are
  independent questions with independent answers.
- **Verifying `## Done criteria`** at the end of a phase, when the criteria
  are separable (the CLI contract, the JSON envelope, the exit codes).
- **Reviewing a finished phase** before its commit.

Do not fan out to *implement* phases or tasks concurrently. The linear chain
is a correctness property of the plan — phase N was written assuming N-1
landed — and parallel writers also race on the very files that record
progress: the checkboxes, the frontmatter `status`, `INDEX.md`, and the
append-only `CHANGELOG.log`.

If concurrent work is genuinely wanted, the unit of concurrency is a *plan*,
not a phase: separate plans, separate indices, separate git worktrees. This
repo is normally worked on through worktrees under `.herdr/worktrees/`, which
already gives each line of work its own checkout.

## Adding a skill

1. `mkdir .agents/skills/<name>` and write `SKILL.md` with `name:` (matching
   the directory) and `description:` frontmatter. The description is the only
   thing a model sees when deciding whether to load the skill — write it as
   the trigger condition, listing the phrasings a user would actually say,
   the way the existing four do.
2. `just wire`.
3. If it is a lifecycle entry point, add a `.claude/commands/<name>.md` that
   pre-loads its context, and add a row to the routing table above.
4. `just harness`.
