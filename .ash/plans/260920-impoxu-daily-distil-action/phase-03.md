---
id: 260920-impoxu
slug: daily-distil-action
phase: 3
status: Done
---

# Phase 3 — Teach the harness about its own robot

## Goal
GOAL-003: Write the unattended mode into the documents that govern it — the
`/distil` command, `plan-learnings` Step 6, `.agents/README.md` — and stop the
robot's daily pull request from corrupting the measurement that grades
`create-pr`.

## Why this phase exists
Phase 2 makes the robot real, and a real robot is a second author in a corpus
that currently assumes every artifact has a human behind it. Two things break
quietly at that moment: `create-pr`'s `evidence:` counts merged pull requests
carrying a `Plan:` footer, and a daily robot pull request entering that
denominator would read as a collapsing conformance rate for a skill that did
nothing wrong (LESSON-012 in reverse — the artifact is still the right thing to
grade, but the population is no longer only ours). And the rule that the
unattended pass must propose rather than write a skill lives, today, only in a
sentence addressed to a human who typed `/distil`. Doing this before the
workflow exists would be documenting a thing that is not there; doing it after
Phase 2 is documenting exactly what shipped.

## Steps
- [x] TASK-019: `.agents/skills/create-pr/SKILL.md` — narrow the `evidence:`
      command's window to exclude the robot author (`gh pr list --author` /
      a `select(.author.login != ...)` filter on the JSON it already asks for),
      and add a one-line comment in the skill body saying why the exclusion
      exists. Why: an unexplained filter in a measurement is indistinguishable
      from gaming it; the reason belongs next to it.
- [x] TASK-020: `scripts/ash.sh` — confirm the narrowed evidence command still
      prints two integers and that `skills` reports `create-pr` at its true
      rate; adjust only if the new command needs a variable the runner does not
      set. Why: `skill.evidence-failed.create-pr` is a warning, not an error,
      so a broken evidence command degrades silently into a dash in the table.
- [x] TASK-021: `.claude/commands/distil.md` — add a short `## Unattended mode`
      section: the run may edit existing skills but never create one; proposals
      go in the pull request body; nothing outside `.ash/` and
      `.agents/skills/` may be touched; `LEARNINGS.md` is append-only. Why: the
      workflow passes `/distil` verbatim (CON-001), so this file *is* the
      robot's instruction set — the rules have to be in it, not in the YAML
      that calls it.
- [x] TASK-022: `.agents/skills/plan-learnings/SKILL.md` — extend Step 6's
      point 4 ("Propose, do not write") with the unattended destination: when
      there is no user in the loop, a proposal is a section in the pull request
      body, and the matching `**Gap:** answered — ...` line is still written on
      the issue. Why: LESSON-014 — the answer needs somewhere to live, or the
      gap section re-asks forever, and an unattended run has no conversation to
      answer into.
- [x] TASK-023: `.agents/README.md` — add the routing row ("distillation, on a
      schedule" → `.github/workflows/distil.yml`) and one line under the
      lifecycle diagram naming the workflow as the unattended entry point to
      the same Step 6 pass. Why: the README is where someone looks to find out
      what runs this repo's harness; an entry point that only exists in
      `.github/` is one a reader has no reason to look for.
- [x] TASK-024: `just wire && just harness` — re-project the edited skills and
      confirm the corpus is clean. Why: LESSON-007 — a skill edited in
      `.agents/` and not wired never loads, and the ironic failure mode here is
      that the robot would read the stale copy.

## Trade-offs & risks
- TASK-019 is a measurement change made by the same plan that creates the thing
  being measured around, which is exactly the shape of a metric being bent to
  look good. It is defensible only because the exclusion is of a *different
  author*, not of inconvenient results, and because the alternative — letting a
  daily robot pull request dominate a window of 100 — measures the robot's
  discipline rather than the skill's.
- ASSUMPTION-003: the robot's pull requests are attributable by author
  (`github-actions[bot]`, or whatever `bot_name` resolves to). If they are not
  cleanly distinguishable, the fallback is to filter on the `harness/distil`
  head branch instead.
- Documenting the rules in `.claude/commands/distil.md` makes them instructions
  to a model, which LESSON-008 warns is the weakest kind of enforcement. Two of
  the four rules are mechanized anyway by Phase 1's `verify` (paths,
  append-only); the other two — never create a skill, proposals in the body —
  stay prose, and that is stated rather than papered over.

## Done criteria
- TEST-012: `scripts/ash.sh skills` reports `create-pr` with a plausible rate
  and no `skill.evidence-failed` finding, before and after a robot pull request
  exists.
- TEST-013: `just harness` exits 0; `agents-wire.sh --check` reports the two
  edited skills as wired.
- TEST-014: `.claude/commands/distil.md` states all four unattended rules, and
  `plan-learnings` Step 6 names the pull request body as a proposal's
  destination.
- TEST-015: `just qc` is green.
- Confirmable only after a live run: TEST-016 — the first merged distillation
  pull request does not move `create-pr`'s conformance rate.
