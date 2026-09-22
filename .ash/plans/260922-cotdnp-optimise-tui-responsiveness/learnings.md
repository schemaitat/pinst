---
id: 260922-cotdnp
slug: optimise-tui-responsiveness
updated: 2026-09-22
areas: [tui, events, upgrades, harness]
issue_count: 4
---

# Learnings — optimise TUI responsiveness (260922-cotdnp)

## Source
- Plan: `./README.md`
- Basis: session context and implementation log
- Logs consulted: `logs/20260922T185139Z-gpt-5.6-sol.log`

## Summary
All four phases shipped with the full quality gate green and release
measurements matching the startup baseline while showing no idle output after
startup. The main implementation surprises were command-contract mistakes and
environmental setup, not architectural reversals.

## Issues

### ISSUE-001: Cargo test was given multiple positional filters
**What happened:** `cargo test --locked app::tests core::docs::search::tests
--no-fail-fast` failed, and a later validation accidentally repeated the same
multi-filter shape.
**Root cause:** `cargo test` accepts one positional `TESTNAME`; additional
test-binary arguments belong after `--`, and libtest still does not interpret
multiple bare strings as multiple independent filters.
**Fix applied:** Ran the complete suite with `cargo test --locked
--no-fail-fast`; focused runs used one filter at a time.
**Recommendation:** Use one filter per invocation, or run the full suite when
several modules need coverage. Put test-binary arguments after `--`.
**Skill:** none
**Distilled:** promoted as LESSON-040.
**Gap:** answered — this is a Rust CLI contract, not a missing repository
workflow skill.

### ISSUE-002: Git had no author identity in the worktree
**What happened:** The first phase commit failed with `Author identity
unknown`.
**Root cause:** This worktree had neither local nor inherited `user.name` and
`user.email` configuration.
**Fix applied:** Reused the repository's existing author identity from
`git log` and configured it locally.
**Recommendation:** Before the first autonomous commit, check that
`git var GIT_AUTHOR_IDENT` succeeds; if it does not, derive the intended
identity from repository history instead of inventing one.
**Skill:** none
**Distilled:** declined — this was an environment-specific setup omission and
the repository history supplied an unambiguous repair.
**Gap:** answered — local Git identity is machine setup, not a missing
repository workflow skill.

### ISSUE-003: An interrupted patch had already landed
**What happened:** A long patch operation was reported as interrupted, but
inspection showed that its changes and tests were already present.
**Root cause:** The operation result and the filesystem mutation crossed the
interruption boundary at different times.
**Fix applied:** Checked the target file and `git status` before retrying, then
continued from the actual tree.
**Recommendation:** After any interrupted write, inspect the artifact and diff
before replaying the operation.
**Skill:** plan-implement
**Distilled:** merged into LESSON-038.

### ISSUE-004: A valid pinst finding exit stopped the benchmark chain
**What happened:** The release benchmark used `&&`; `pinst list --json`
completed normally with exit 3 for actionable machine findings, so the
pseudo-terminal measurements did not run.
**Root cause:** The benchmark treated every non-zero status as execution
failure instead of following pinst's documented status-code contract.
**Fix applied:** Kept the valid 0.85-second measurement and ran the PTY
measurements separately.
**Recommendation:** When scripting pinst, accept exit 3 as a completed result
and inspect the JSON envelope; reserve exit 1 and 2 for failure and usage
handling.
**Skill:** pinst
**Distilled:** declined — the pinst skill already states this exact exit-code
contract; the issue was failing to apply it.
