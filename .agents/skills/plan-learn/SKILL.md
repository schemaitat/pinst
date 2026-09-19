---
name: plan-learn
description: Write or update an ADR-style learning summary for a plan at .ash/plans/<id>-<slug>/learnings.md, promoting lessons that generalize into .ash/LEARNINGS.md, capturing what went wrong during implementation and how to fix, avoid, or improve it next time. Always invoke this right after plan-implement finishes a run (completed, aborted, or blocked) — implementation is not done until the learnings are written. Also use it on demand for a past plan (e.g. "what did we learn from 260919-qwerty", "write up the learnings for the mlflow plan", "post-mortem this implementation"), in which case it reads the plan's implementation logs instead of live session context.
---

# Plan Learn Skill

Captures what actually happened while implementing a plan — specifically
the things that went wrong, were surprising, or took a workaround — as an
ADR-style record at `.ash/plans/<id>-<slug>/learnings.md`, sitting
beside the plan it came from. A plan document records what was *decided*;
this file records what was *learned by trying it*, so the next person (or
the next plan) doesn't rediscover the same problem from scratch.

Learnings are written at two levels, and the split is what keeps this
memory useful as it grows:

- **`.ash/plans/<id>-<slug>/learnings.md`** — raw and specific. Every
  issue this plan hit, in full detail, next to the plan that caused it.
- **`.ash/LEARNINGS.md`** — distilled and general. Only the lessons that
  apply beyond their own plan. This is the file read *before* new work
  starts, so it has to stay short and high-signal; if every plan-specific
  gripe were promoted here, its signal-to-noise would decay with every
  plan and people would stop reading it.

There is exactly **one** learnings file per plan, not one per run. A plan
implemented across several sessions accumulates entries in the same file —
never overwrite it; read what's there first and add to it.

______________________________________________________________________

## Step 1 — Identify the plan and the source of truth

- If a plan index/slug is given (or obvious from context — you just ran
  `plan-implement`), use it directly.
- Otherwise, ask which plan, the same way `plan-implement` does — don't
  guess which plan a standalone "write up the learnings" request refers to
  if more than one is plausible.

Then pick your source, based on how you were invoked:

- **Right after `plan-implement`** (the common case): the session's own
  context is the richest source — it has the actual reasoning, not just
  the log's compressed summary. Use it as the primary source, and cross-
  check it against this run's log file for anything the conversation
  glossed over.
- **On demand, standalone**: there is no fresh context to draw from, so
  read every file under `.ash/plans/<id>-<slug>/logs/` in chronological
  order (oldest to newest) to reconstruct what happened across all runs.
  The `task_failed`, `phase_failed`, and `issue` events are exactly what
  you're looking for; `task_done`/`phase_done`/`commit` fill in the
  surrounding story.

## Step 2 — Read what's already been learned

Check whether `.ash/plans/<id>-<slug>/learnings.md` already exists. If
it does, read it — you're appending, not restarting. Note the highest
existing `ISSUE-NNN` number so new entries continue the sequence, and skip
anything already captured rather than writing a near-duplicate entry for it.

Read `.ash/LEARNINGS.md` too, for the same reason: a lesson already
recorded there doesn't need promoting a second time, and seeing a plan hit
something that's *already* a known general lesson is itself worth noting —
it means the lesson isn't being applied, which is a different problem from
not having learned it yet.

## Step 3 — Extract issues

An "issue" here is anything that, with hindsight, is worth someone knowing
before they hit it too: a failed task, a wrong assumption the plan made, a
flaky test, an environment quirk, a design choice that had to be reversed
mid-implementation. A clean, uneventful phase is not an issue — don't
manufacture one just to have something to write.

For each issue, work out four things, which is the ADR habit applied to a
single problem instead of a whole plan:

- **What happened** — the concrete symptom, not a vague gesture at it.
- **Root cause** — why it happened, one level deeper than the symptom.
- **Fix applied** — what actually got it unstuck this time (or "unresolved
  — abandoned" if it wasn't fixed).
- **Recommendation** — the decision for next time: how to avoid it
  entirely, catch it earlier, or handle it faster if it recurs. This is
  the part most worth writing well, since it's the payoff for the whole
  file.

## Step 4 — Write or update the plan's learnings file

`.ash/plans/<id>-<slug>/learnings.md`:

```markdown
---
id: <id>
slug: <slug>
updated: <YYYY-MM-DD>
areas: [<area>, <area>]
issue_count: <N>
---

# Learnings — <plan goal> (<id>-<slug>)

## Source
- Plan: `./README.md`
- Basis: session context | implementation log(s)
- Logs consulted: <path(s), or "none — clean run">

## Summary
<2-4 sentences: what was implemented, the overall outcome (completed,
partially, aborted), and the headline lesson if there's one worth
surfacing above the issue list.>

## Issues

### ISSUE-001: <short title>
**What happened:** <symptom>
**Root cause:** <why>
**Fix applied:** <what got it working, or "unresolved">
**Recommendation:** <how to avoid/catch/fix faster next time>

### ISSUE-002: <short title>
...
```

If a run genuinely produced no issues, still write the file with an empty
`## Issues` section (`None — implementation went as planned.`) rather than
skipping the file — a missing file and a clean run should look different
to someone checking later. `scripts/ash.sh check` enforces exactly that
distinction: a plan marked `Done` with no `learnings.md` is a finding
(`plan.learnings-missing.<plan>`), and `just qc` runs it.

When appending to an existing file, insert new `ISSUE-NNN` entries after
the existing ones (continuing the numbering) and update `## Summary` if
the new run changes the overall picture; leave prior entries untouched.
The `id` in frontmatter must match the plan's directory; `scripts/ash.sh
check` reports it as `plan.id-mismatch.<plan>` if it drifts.

## Step 4b — Promote what generalizes to `.ash/LEARNINGS.md`

Now re-read the issues you just wrote and ask, for each: **would this have
been worth knowing before a completely different plan started?** If yes,
it belongs in `.ash/LEARNINGS.md` too. If it's specific to this plan's
particular code or circumstances, leave it where it is — the per-plan file
already has it, and promoting everything is how this file becomes noise.

Signals a lesson generalizes: it's about the toolchain, the environment,
the repo's conventions, or the shape of the work (estimation, sequencing,
testing strategy) rather than about one function's behavior. A lesson that
a *second* plan has now hit is essentially always worth promoting — repeat
occurrences are the strongest evidence that something is systemic.

`.ash/LEARNINGS.md` is append-only with its own `LESSON-NNN` sequence,
independent of any plan's `ISSUE-NNN` numbering:

```markdown
# Distilled learnings

Lessons that generalize beyond the plan that produced them. Read this
before planning new work.

### LESSON-001: <short imperative title>
**Lesson:** <what to do or avoid, stated so it's actionable without
reading the source plan>
**Why:** <the underlying reason — the thing that makes it true in general,
not just the anecdote it came from>
**Seen in:** 260919-qwerty-add-mlflow-experiment-tracking (ISSUE-002)
```

When a lesson already in the file recurs in a new plan, don't add a second
entry — append the new plan to its `Seen in:` line. The count of sources
on one lesson is a useful signal in itself: it marks the problems this
codebase keeps re-creating, which are the ones worth fixing structurally
rather than remembering harder.

## Step 5 — Report back

Run `just harness` to confirm the corpus is clean, then tell the user both
file paths, how many issues were recorded (or that it was a clean run), and
which lessons — if any — were promoted to `.ash/LEARNINGS.md`. Don't paste the full files into the conversation;
they're meant to be read later, by a human or by a future planning
session, not re-consumed immediately.
