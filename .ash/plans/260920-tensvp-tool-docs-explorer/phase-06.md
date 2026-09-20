---
id: 260920-tensvp
slug: tool-docs-explorer
phase: 6
status: Proposed
---

# Phase 6 — Publish the contract and make it browsable

## Goal
**GOAL-006**: tell the two audiences it exists — the agent, through the skill
that is this CLI's machine contract, and the human, through the README and a
Docs tab in the TUI — because a catalogue nothing knows to query is a
catalogue nobody reads (REQ-001).

## Why this phase exists
Everything works by the end of phase 5 and nothing announces it. The skill is
what an agent loads before driving pinst, so until `docs` is documented there
the feature is discoverable only by running `pinst --help` — which is exactly
the "think first, then call the tool" loop this plan set out to remove. It
comes last because documenting a contract before it is final documents a draft,
and LESSON-009 is specifically about records that describe a world which has
moved on.

## Steps
- [ ] TASK-033: `.agents/skills/pinst/SKILL.md` — a `docs` section in the
      commands table plus the per-action `items[]` shapes, the exit codes
      (`show`'s 0/2/3, `search`'s always-0, `adopt`'s refusals), and one line
      on *when* to reach for it: before running an unfamiliar tool, and before
      assuming a tool is absent.
- [ ] TASK-034: `README.md` — `pinst docs search|show|dump|status|adopt` in
      the commands table, and a short section on the catalogue: where pages
      live, that they are embedded, and how a draft becomes authored.
- [ ] TASK-035: `src/app.rs` — `Tab::Docs` plus the catalogue state it needs,
      alongside `Overview`, `Health` and `Upgrades`.
- [ ] TASK-036: `src/ui/docs.rs` (new) and `src/ui/mod.rs` — render the
      selected tool's page: `what`, `when`, recipes, gotchas, with `draft`
      marked. Why in the TUI at all: the dashboard is already the human's
      answer to "what is on this machine", and "what is it for" is the next
      question every time.
- [ ] TASK-037: `src/event.rs` — the existing `/` search filters the Docs tab
      by the same ranking phase 4 built, so the TUI and the CLI cannot
      disagree about what matches.
- [ ] TASK-038: `just wire` then `just review` — the skill changed, and the
      harness checks that a changed skill is projected into `.claude/skills/`
      (LESSON-007) and that its `produces`/`evidence` contract still holds.

## Trade-offs & risks
- The TUI tab (TASK-035 to TASK-037) is the one droppable piece of this plan.
  It serves the human, while the stated goal is the agent; if it is running
  late it can be split into its own plan without anything else losing value.
  It is sequenced last for exactly that reason.
- **RISK-004**: the skill is where scope creep would show up first, as a
  paragraph promising man-page rendering or upstream sync. What goes in is the
  contract for what shipped, nothing else.
- Reusing phase 4's ranking in the TUI couples the two surfaces. Accepted
  deliberately: two matchers would be two behaviours to explain, and the
  ranking is already the thing under test.

## Done criteria
- **TEST-013**: `just review` is green — the skill is wired into
  `.claude/skills/`, the corpus checks pass, and `scripts/ash.sh check`
  reports no findings.
- **TEST-014**: every `docs` action and exit code documented in
  `SKILL.md` matches what the binary does, verified by running each one.
- **TEST-015**: the TUI opens on the Docs tab, renders a page, and `/`
  filters it; the filter function is unit-tested against the same fixtures as
  the CLI ranking.
- `just qc` is green, and `.ash/INDEX.md` is regenerated with this plan's
  final status.
