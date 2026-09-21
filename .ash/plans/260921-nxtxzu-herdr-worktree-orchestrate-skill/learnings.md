---
id: 260921-nxtxzu
slug: herdr-worktree-orchestrate-skill
updated: 2026-09-21
areas: [agents, herdr, orchestration]
issue_count: 6
---

# Learnings — Herdr worktree orchestration as a portable skill (260921-nxtxzu-herdr-worktree-orchestrate-skill)

## Source

- Plan: `./README.md`
- Basis: session context
- Logs consulted:
  `logs/20260921T214206Z-composer.log`,
  `logs/20260921T214344Z-gpt-5-6-sol.log`

## Summary

Both phases completed. The repository now ships a harness-agnostic
`orchestrate` skill and optional command, and two live Herdr runs proved the
sibling-pane and isolated-worktree paths through semantic resolution. The
smoke work exposed three details worth encoding in the skill itself:
worktrees created from a linked checkout must target the parent source
workspace, a timeout can occur before prompt delivery, and missing integration
does not make one observed lifecycle state authoritative.

## Issues

### ISSUE-001: The skill became a checked asset one phase before its planned wiring
**What happened:** Phase 1 added `.agents/skills/orchestrate/SKILL.md`, while
the plan placed `just wire` in Phase 2 after the command. The repository
immediately saw a seventh skill, so leaving the source unwired and the
real-corpus count at six would have made Phase 1 fail `just harness`/`just qc`.
**Root cause:** The phase boundary followed the portable runtime content but
did not include the repository packaging invariants triggered by creating an
asset under `.agents/`.
**Fix applied:** The skill projection and six-to-seven inventory update landed
with Phase 1. Phase 2 then projected the command after it existed.
**Recommendation:** Any phase that first creates a `.agents` asset must also
wire that asset and update exact inventory assertions in the same phase, even
when later phases add related assets.
**Skill:** plan-write

### ISSUE-002: The first commit had no author identity
**What happened:** `git commit` failed with "Author identity unknown" after
Phase 1 was staged and verified.
**Root cause:** Neither repository-local nor global Git configuration supplied
`user.name` or `user.email` in this environment.
**Fix applied:** Recent repository commits established the intended author and
noreply address. The commit used matching `GIT_AUTHOR_*` and
`GIT_COMMITTER_*` variables for that command without changing persistent
configuration.
**Recommendation:** On an identity failure, use repository history to recover
an established identity and apply it per command; if history is ambiguous,
ask. Do not silently write global Git configuration.
**Skill:** conventional-commits

### ISSUE-003: A linked worktree cannot be the source of another Herdr worktree
**What happened:** The first smoke `herdr worktree create` returned
`linked_worktree_source`. A retry that supplied both the discovered parent
workspace and its cwd then failed usage validation because those selectors are
mutually exclusive.
**Root cause:** The initial procedure assumed the caller checkout was always a
valid creation source and treated `--workspace` plus `--cwd` as complementary
rather than alternative selectors.
**Fix applied:** The skill now runs `herdr worktree list --cwd "$PWD"`, reads
the response's source workspace/path, and passes the open source workspace
alone. It also logs recoverable control failures before inspecting live state.
**Recommendation:** Discover Herdr's repository source before every worktree
mutation, and use exactly one returned source selector.
**Skill:** orchestrate
**Distilled:** declined — Herdr-specific command topology, now encoded and
smoke-tested in the skill that owns it.

### ISSUE-004: A caller timeout can expire before a prompt is delivered
**What happened:** A deliberately one-millisecond `agent prompt --wait`
timeout returned `timeout`; `agent get` and `agent read` showed the child idle,
with no delegated prompt in its transcript. A later one-second timeout showed
the prompt present and work underway.
**Root cause:** The timeout covers submission as well as waiting, so the same
error code can describe "never submitted" or "submitted and still working".
Logging `prompt_sent` before evidence therefore overstates what happened.
**Fix applied:** The skill now appends `prompt_sent` only after command or
transcript evidence, inspects every timeout, and permits one exact retry only
when the prompt's absence and lack of work are both proven.
**Recommendation:** Treat timeout as an observation boundary, not a delivery
verdict; inspect before deciding whether to wait, retry, or nudge.
**Skill:** orchestrate
**Distilled:** declined — this is the installed Herdr prompt contract and is
now the central recovery rule in the skill.

### ISSUE-005: Missing integration still produced useful but briefly stale states
**What happened:** Cursor's optional Herdr integration was not installed, yet
the smoke runs observed `idle`, `blocked`, and `done`. Immediately after an
approved verification command, one `agent wait` still returned `blocked`
while the transcript already showed the completed report; a later `get`
advanced to `done`.
**Root cause:** Lifecycle classification can lag terminal output, and without
the optional integration no single state sample is strong enough to establish
semantic completion.
**Fix applied:** The skill records integration status and requires `get`,
`read`, artifacts, acceptance checks, and blocker inspection together. The
smoke result was marked resolved only after the file and git status were
independently verified and the later state became `done`.
**Recommendation:** Use lifecycle state to choose the next observation, never
as the completion proof; re-read state after transcript evidence changes.
**Skill:** orchestrate
**Distilled:** declined — this is a Herdr/Cursor integration behavior already
captured in the owning skill's state machine.

### ISSUE-006: A real-corpus test asserted that an issue tally stayed zero
**What happened:** After this learnings file truthfully named
`conventional-commits` on ISSUE-002, `just qc` failed
`issues_are_tallied_from_the_skill_lines_plans_record`; the test expected that
skill's issue count to remain exactly zero.
**Root cause:** The test used the growing production corpus to prove tallying
while also asserting a temporary absence in that corpus. Appending a valid
issue changed data, not behavior, but broke the test.
**Fix applied:** The test now asserts positive tallies for `plan-write`,
`conventional-commits`, and `orchestrate`; fixture tests remain the place for
exact zero/positive behavior.
**Recommendation:** Against append-only real corpora, assert durable
structural facts. Put exact absence and count expectations in synthetic
fixtures whose inputs the test owns.
**Skill:** none
**Gap:** answered — this was a brittle Rust test, not recurring work that a
skill should own.
