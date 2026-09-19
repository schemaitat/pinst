---
name: plan-write
description: Create and persist an implementation plan as ADR-style docs under .ash/plans/. Use when the user wants to create, formalise, save, write, or record a plan for a feature, refactor, upgrade, migration, data change, architecture, design, or infrastructure change.
---

# Plan Write Skill

Take current session's discussion, write directly as ADR-style planning docs
under `.ash/plans/<id>-<slug>/` — no intermediate structured file ever
written to disk. `.ash/plans` is always the execution substrate — these files
are read and ticked off directly as implementation progresses.

Every file must read like Architectural Decision Record: captures not just
**what** is being done and **how**, but also **why** — why this approach chosen,
why alternatives rejected, why each phase structured the way it is.

`.ash/` is a growing, searchable memory of how this codebase has been
changed. Its layout, which this skill and `plan-implement`/`plan-learnings`
share:

```
.ash/
├── INDEX.md          # generated: every plan — index, status, date, areas, summary
├── LEARNINGS.md      # distilled cross-plan lessons; read before planning new work
├── CHANGELOG.log     # chronological feed of shipped work (logfmt, one line each)
└── plans/
    └── 260919-qwerty-add-mlflow-experiment-tracking/
        ├── README.md      # ADR index (frontmatter + decision record)
        ├── phase-01.md    # per-phase ADR (multi-phase plans only)
        ├── learnings.md   # what went wrong implementing it, and what to do instead
        └── logs/          # one JSONL log per implementation run
```

Before writing a new plan, read `.ash/LEARNINGS.md` — it's the accumulated
record of what has actually gone wrong here before, and it's cheapest to
account for while the approach is still being decided.

______________________________________________________________________

## Step 1 — Gather & label

Confirm plan's goal (one sentence) if not already clear from session. Explore
codebase as needed to identify affected files, existing patterns, dependencies.

This survey is the one part of the lifecycle worth fanning out for. The
questions it answers — "which modules touch the manifest schema", "where is
version probing done", "what already handles config drift" — are independent
of each other, read-only, and merge cleanly, so they can be delegated in
parallel. Everything downstream is deliberately sequential; see "Delegating"
in `.agents/README.md`.

Extract + label session's content using identifier categories below. These
identifiers are in-context reasoning scaffold only — never written to disk as
own file; feed directly into ADR sections written in later steps.

- `REQ-NNN` functional requirements, `SEC-NNN` security requirements, `CON-NNN`
  constraints, `GUD-NNN` guidelines, `PAT-NNN` patterns to follow
- `GOAL-NNN` per-phase goal, `TASK-NNN` per-phase tasks
- `ALT-NNN` alternatives considered + why rejected
- `DEP-NNN` dependencies (libraries, frameworks, components relied on)
- `FILE-NNN` affected files + what changes
- `TEST-NNN` tests/verification
- `RISK-NNN` risks, `ASSUMPTION-NNN` assumptions

Every identifier must be **declared exactly once**. Same identifier may be
referenced any number of times afterward.

**Linearity rule:** phases must form a strictly linear chain — phase N depends
only on phase N-1, never on more than one predecessor and never in parallel with
another phase. Within a phase, `TASK-NNN` items must also form a strictly linear
chain — each task depends only on the immediately preceding `TASK-NNN` in that
phase. If the session's discussion suggests branching or parallel phases/tasks,
flatten them into a single linear order before continuing to later steps; never
write a plan whose phases or tasks branch. Step 5b re-checks this before
anything is written to disk.

______________________________________________________________________

## Step 2 — Mint an id and a slug

Every plan gets a permanent id of the form `<yymmdd>-<six letters>`, e.g.
`260919-qwerty`. It is assigned once and never changed — it's how commits and
PRs trace back to the plan that authorized them (see "Carrying the id
forward" below), so it must stay stable even if the plan is later abandoned.

1. Mint it:

   ```sh
   scripts/ash.sh new-id        # -> 260919-qwerty
   ```

   (The `/plan` command already runs this and puts the result in context.)
2. Create a short kebab-case slug from the goal identified in Step 1 (3–5
   words, lowercase, hyphens only). Example: "Add MLflow experiment
   tracking" → `add-mlflow-experiment-tracking`.
3. Combine as `<id>-<slug>`, e.g.
   `260919-qwerty-add-mlflow-experiment-tracking`.

**Never derive an id by counting existing plans.** The obvious scheme —
highest number plus one — requires asking the whole corpus a question, and
work here happens in parallel git worktrees branched from the same commit.
Two sessions both compute the same "next" value, both use it, and the
collision only surfaces at merge, when the identifier is already written into
commit footers and cannot be changed without breaking exactly the traceability
it exists to provide. The random half of the id needs no coordination, so
there is nothing to collide over.

The date half is `yymmdd` rather than the friendlier `ddmmyy` for one
reason: most-significant-first means string order *is* date order, so a plain
`ls .ash/plans/` and the generated `INDEX.md` both come out chronological
without a sort key.

All files for this plan live under `.ash/plans/<id>-<slug>/`, always a
folder — single-phase and multi-phase plans share one layout so downstream
consumers never branch on "is it a file or a directory".

Everything about one plan is co-located in that folder: the ADR docs, the
implementation logs `plan-implement` writes under `logs/`, and the
`learnings.md` that `plan-learnings` writes when implementation finishes. One
unit of work, one directory — nothing to keep in sync across the tree, and
a single `grep -r` there answers "what do we know about this work".

Also note, for the frontmatter written in Steps 4/5, which **areas** of the
codebase this plan touches (2–4 short tags like `cli`, `configs`,
`manifest`). Slugs alone are a weak search key; areas are what let a future
session find this plan by subject rather than by remembering its name.

______________________________________________________________________

## Step 3 — Determine single-phase vs. multi-phase

- **Single-phase plan** (only one Implementation Phase identified in Step 1's
  GOAL/TASK labelling): write **`.ash/plans/<id>-<slug>/README.md`** only,
  using the full ADR template from Step 5. Skip Step 4.
- **Multi-phase plan** (two+ Implementation Phases): write
  `.ash/plans/<id>-<slug>/README.md` as the index plus one `phase-NN.md`
  per phase. Continue with Step 4.

______________________________________________________________________

## Step 4 — Multi-phase layout

### Main index file: `.ash/plans/<id>-<slug>/README.md`

This is ADR index. Must capture full decision record for feature.

```markdown
---
id: <id>
slug: <slug>
status: Proposed          # Proposed | In Progress | Done
created: <YYYY-MM-DD>
updated: <YYYY-MM-DD>
areas: [<area>, <area>]
summary: <one line, the same sentence used in INDEX.md>
files_touched: [<path>, <path>]
---

# <goal>

## Context
<2–4 sentences describing the problem, the current state of the codebase, and
why this work is needed now. Drawn from REQ/SEC/CON identifiers and the plan introduction.>

## Decision
<The chosen approach in 3–6 sentences. State clearly what was decided and
why this option was preferred: what properties it has that the alternatives lack,
what trade-offs were accepted, and what constraints shaped the choice.>

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| <ALT-001 label> | <ALT-001 reason> |
| <ALT-002 label> | <ALT-002 reason> |

## Consequences
<What becomes easier, harder, or different as a result of this decision.
Include known risks (RISK identifiers), assumptions (ASSUMPTION identifiers),
and dependencies (DEP identifiers) that affect future work.>

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | <GOAL-001 title> | [phase-01.md](phase-01.md) | Proposed |
| 2 | <GOAL-002 title> | [phase-02.md](phase-02.md) | Proposed |
| … | … | … | … |

## Affected Files
<verbatim FILE-NNN list from the plan>

## Open Questions
<Any unresolved ASSUMPTION identifiers, or "None." if all are resolved>
```

### Per-phase files: `.ash/plans/<id>-<slug>/phase-<NN>.md`

Use zero-padded two-digit numbers (`01`, `02`, …).

Each phase file also ADR-like: must justify why this slice structured as it is.

```markdown
---
id: <id>
slug: <slug>
phase: <N>
status: Proposed          # Proposed | In Progress | Done
---

# Phase <N> — <GOAL-NNN title>

## Goal
<GOAL-NNN text: what this phase achieves in one sentence and why it is the right
next slice — what value or unblocking it delivers before the following phases can start.>

## Why this phase exists
<1–3 sentences explaining the rationale for this slice boundary: what would go
wrong if this work were merged into the previous or next phase, or why this
vertical slice was chosen over a different cut. Must name only the single
preceding phase as this phase's dependency (phase N depends only on phase
N-1) — never more than one predecessor.>

## Steps
<Each TASK-NNN item from the phase table rendered as `- [ ] TASK-NNN: <file> — <change>`.
For non-obvious steps, add an inline "Why:" note explaining the reasoning.
Tasks must form a single unbroken chain: each TASK-NNN depends only on the
immediately preceding TASK-NNN in this phase, never on more than one prior task.
Steps start unchecked; tick them off as work progresses and flip `## Status` to `Done` on approval.>

## Trade-offs & risks
<RISK/ASSUMPTION identifiers that apply to this phase. What was accepted or deferred,
and why. Note any shortcuts taken and what would be needed to address them later.>

## Done criteria
<TEST-NNN items that apply to this phase. What must be true before this phase is
considered complete.>
```

**Mapping Step 1 labels → ADR:**

- Goal identified in Step 1 → ADR `## Decision` summary and file slug
- `REQ`/`SEC`/`CON`/`GUD`/`PAT` identifiers → ADR `## Context` and
  `## Consequences`
- `GOAL`/`TASK` identifiers (one set per phase) → ADR phases
- `ALT` identifiers → ADR `## Alternatives Considered`
- `DEP` identifiers → ADR `## Consequences`
- `FILE` identifiers → ADR `## Affected Files`
- `TEST` identifiers → per-phase `## Done criteria`
- `RISK`/`ASSUMPTION` identifiers → ADR `## Consequences` and per-phase
  `## Trade-offs & risks`

______________________________________________________________________

## Step 5 — Single-phase file layout

When plan has only one Implementation Phase,
`.ash/plans/<id>-<slug>/README.md` must follow full ADR structure:

```markdown
---
id: <id>
slug: <slug>
status: Proposed          # Proposed | In Progress | Done
created: <YYYY-MM-DD>
updated: <YYYY-MM-DD>
areas: [<area>, <area>]
summary: <one line, the same sentence used in INDEX.md>
files_touched: [<path>, <path>]
---

# <goal>

## Context
<Why this work is needed — from REQ/SEC/CON identifiers and the plan introduction>

## Decision
<What was decided and why — from GOAL-001 and the plan introduction>

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| <ALT-001> | <reason> |

## Consequences
<RISK, ASSUMPTION, DEP identifiers that shape future work>

## Steps
<Each TASK-NNN item rendered as `- [ ] TASK-NNN: <file> — <change>`, with inline
"Why:" notes for non-obvious changes. Tasks must form a single unbroken chain:
each TASK-NNN depends only on the immediately preceding TASK-NNN, never on more
than one prior task.
Steps start unchecked; tick them off as work progresses and flip `## Status` to `Done` on approval.>

## Trade-offs & risks
<Shortcuts taken (from RISK identifiers) and what would address them later>

## Affected Files
<FILE-NNN list>

## Open Questions
<Unresolved ASSUMPTION identifiers, or "None.">

## Done criteria
<TEST-NNN items>
```

### Frontmatter rules

The frontmatter is what makes this a searchable corpus rather than a pile
of prose, so it has to stay trustworthy:

- **`id` must match the directory it lives in.** It needs no quoting:
  `260919-qwerty` is not numeric, so YAML cannot mangle it the way it
  silently turned a zero-padded `0001` into the integer `1` under the
  scheme this replaced.
- **`status` lives in frontmatter only.** Don't also add a `## Status`
  section; two copies of a mutable field is two copies to drift apart.
  For multi-phase plans, each `phase-NN.md` frontmatter holds that
  phase's authoritative status, and the README's `## Phases` table
  mirrors it for readability — whoever flips one flips both in the same
  edit.
- **`summary` is the same sentence that appears in `INDEX.md`**, so the
  index can be regenerated from the files without re-deriving it.
- `areas` and `files_touched` are search keys, not documentation — keep
  them short and literal (real paths, real subsystem names).

______________________________________________________________________

## Step 5b — Validate before writing

Before invoking Write for any file, re-check the linearity rule from Step 1
against the content actually assembled for the ADR docs (not just the original
session discussion — phases/tasks may have shifted while drafting):

- Every phase and, within each phase, every `TASK-NNN` must form a single linear
  DAG: exactly one predecessor and one successor, no branching, no merging, no
  forward references, no cycles.
- Every identifier (`REQ`, `SEC`, `CON`, `GUD`, `PAT`, `GOAL`, `TASK`, `ALT`,
  `DEP`, `FILE`, `TEST`, `RISK`, `ASSUMPTION`) must be declared exactly once.

If either check fails, reorder or flatten the offending phases/tasks and re-run
this check before continuing to Step 6. Never invoke Write while a check is
still failing.

These two are on you: linearity and declared-exactly-once are properties of
the *reasoning*, and no script can tell a well-ordered plan from a badly
ordered one. `scripts/ash.sh check` covers the mechanical half afterwards
(phase numbering, frontmatter, status mirrors) — it is not a substitute for
this step.

______________________________________________________________________

## Step 6 — Write the plan

Use Write tool to create every ADR file determined above under
`.ash/plans/<id>-<slug>/`. Don't modify any existing file outside
`.ash/plans/`, except `.ash/INDEX.md` (Step 7).

After writing, print short summary listing every file created + its path,
and call out the plan's id explicitly (e.g. "Plan ID: 260919-qwerty") so it's easy
to carry into the commits/PRs that implement it.

______________________________________________________________________

## Step 7 — Regenerate the index

`.ash/INDEX.md` is the entry point to the whole corpus — the one file an
agent reads to decide which plans are worth opening. Once there are dozens
of plans, reading every README to find the relevant one stops being
affordable; reading one table and then opening two files stays cheap
forever.

**Never hand-edit it — regenerate it:**

```sh
just index          # or: scripts/ash.sh index
```

That script rewrites the table whole from every `.ash/plans/*/README.md`
frontmatter, sorted by id — which, because ids start with `yymmdd`, is
chronological order. Every field is taken verbatim. If a
value looks wrong in the index, fix the plan's frontmatter and regenerate
rather than patching the table.

Regenerating is mechanical for a reason: a hand-maintained index drifts out
of sync and then confidently reports things that aren't true, which is worse
than having no index at all. `scripts/ash.sh check` treats a stale index as
a finding, and `just qc` runs it, so a forgotten regeneration surfaces as a
failing check rather than as a quietly wrong table.

Then confirm the corpus you just wrote is well-formed:

```sh
just harness        # exit 0 clean, 3 means findings to act on
```

It validates what this skill has been asserting in prose all along — quoted
indices that match their directory, required frontmatter keys, legal
`status` values, no `## Status` section duplicating frontmatter, phase files
numbered `01..N` with no gaps, and each phase's status agreeing with the
README's `## Phases` table. Fix anything it reports before handing the plan
back; findings carry the exact remediation.

______________________________________________________________________

## Carrying the id forward

The id assigned in Step 2 is the plan's permanent identifier. Once
implementation starts, every commit and pull request for this plan must
reference it — otherwise the link between a plan and the code it produced is
only findable by memory, and that's exactly the kind of thing this skill
exists to avoid losing.

- **Commits**: add a trailing footer line `Plan: <id>-<slug>` (e.g.
  `Plan: 260919-qwerty-add-mlflow-experiment-tracking`), after any other
  footers like `BREAKING CHANGE:` or attribution lines.
- **Pull requests**: prefix the title with the id in brackets, e.g.
  `[260919-qwerty] Add MLflow experiment tracking`, so it's visible in PR
  lists without opening the description.

This applies for the plan's entire lifetime, not just its first commit — if
work spans several phases and several PRs, every one of them carries the same
id.
