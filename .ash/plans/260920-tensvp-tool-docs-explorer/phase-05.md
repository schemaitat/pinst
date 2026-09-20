---
id: 260920-tensvp
slug: tool-docs-explorer
phase: 5
status: Proposed
---

# Phase 5 — Author the catalogue

## Goal
**GOAL-005**: fill the catalogue — a page for every tool the manifest
declares, each one written for the machine this repo provisions rather than
for tools in general, and each authored page's recipes actually run before it
is called authored (REQ-002, RISK-003).

## Why this phase exists
The commands are worthless empty, and this is the only phase whose output is
prose rather than code. Separating it means the machinery can be reviewed
without reading 26 pages of documentation, and the pages can be reviewed
without re-reading the machinery. It has to follow phase 4 because search is
how the pages get tested: the question "does the catalogue answer this
intention" can only be asked once something can be asked.

## Steps
- [ ] TASK-027: seed every capture-capable tool with `pinst docs adopt`, so
      authoring starts from the tool's real help on this machine rather than
      from memory of what its flags are.
- [ ] TASK-028: author the pages for the tools an agent reaches for daily —
      `ripgrep` (already written), `fd`, `just`, `gh`, `uv`, `delta`, `git`,
      `nvim`, `pinst` itself — promoting each to `status = "authored"` with
      `verified_with` set to the probed version. Each carries `see_also` links
      to the tools it competes or composes with, because "which tool" is half
      the question being asked.
- [ ] TASK-029: author the pages for the local tools no model has priors
      about — `herdr`, `aven`, `opencode`, `claude`. Why these matter most:
      for `git` a model already has decent priors and the page is a
      refinement; for these, the page is the only source there is.
- [ ] TASK-030: write short pages for the tools with no CLI surface —
      `oh-my-zsh`, `zsh-autosuggestions`, `build-essential`, `nvm`,
      `git-credential-manager` — saying what they are, what depends on them,
      and explicitly that there is nothing to invoke. Why bother: "this has no
      command" is a useful answer, and its absence reads identically to an
      unwritten page.
- [ ] TASK-031: query the finished catalogue with the intentions an agent
      actually has — "search a tree", "run a task", "open a PR", "python
      env", "diff", "install a node version" — and fix every miss by adding
      `keywords`, not by loosening the matcher.
- [ ] TASK-032: measure and record the cost: `just dist` before and after the
      catalogue, noting the tarball delta in this phase's completion.
      Why recorded rather than asserted: a number nobody wrote down cannot be
      compared against later, and this is the first thing to check if the
      binary ever gets uncomfortably large (RISK-002).

## Trade-offs & risks
- **RISK-003** is at its peak here: a confidently wrong recipe will be run by
  an agent without a second thought. TASK-028/029's rule — a page is
  `authored` only after its recipes were executed on this machine — is the
  mitigation, and anything unrun stays `draft`, which travels with the page
  through search, show and dump.
- **RISK-001**: `verified_with` is written once at authoring time and nothing
  updates it. `docs status` surfaces the drift; deliberately nothing fails on
  it (GUD-001).
- Some recipes cannot be run safely (anything that writes, installs or opens a
  PR). Those stay in the page but are marked in `does` as unrun, and the page
  stays `draft` unless its remaining recipes were verified — the honest
  answer, rather than quietly promoting on a partial check.
- This phase is where **ASSUMPTION-002** becomes expensive to reverse. That is
  by design: four phases of commands have exercised the format before any bulk
  content is committed to it.

## Done criteria
- **TEST-010**: every manifest tool has a page; `pinst docs status --json`
  reports `page: "none"` for zero tools, and every `authored` page carries a
  `verified_with`.
- **TEST-011**: each of the six intentions in TASK-031 returns the right tool
  first from `pinst docs search`.
- **TEST-012**: the `just dist` tarball delta is recorded in this phase's
  closing note, with the before and after byte counts.
- `just qc` is green, including TEST-001's parse and orphan checks over the
  full catalogue.
