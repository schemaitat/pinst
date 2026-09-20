---
id: 260920-wtburh
slug: harness-in-the-binary
updated: 2026-09-20
areas: [cli, harness, agents]
issue_count: 8
---

# Learnings — the harness moves into the binary (260920-wtburh-harness-in-the-binary)

## Source
- Plan: `./README.md`
- Basis: session context, cross-checked against this run's log
- Logs consulted: `logs/20260920T114819Z-claude-opus-5.log`

## Summary
All five phases completed in one run: 41 tasks, 62 new tests, `scripts/ash.sh`
(1054 lines of bash) replaced by `pinst harness check|index|skills|new-id`.
The port reproduces the incumbent exactly — byte-identical index output,
byte-identical skill report with and without `--transcripts`, identical
finding ids on six deliberately broken fixture corpora — and the release
binary with `cargo` off `PATH` produces the same output as the source build.

The headline lesson is about how that was verified rather than about the port
itself. Running both implementations side by side on *broken* input is what
caught the one real defect (ISSUE-003); every comparison on a healthy corpus
agreed, and would have shipped it. The second-most useful finding was the
opposite shape: a check that greps the source for a finding id was satisfied
by a doc comment that merely *mentioned* the id, so it passed while enforcing
nothing (ISSUE-007).

## Issues

### ISSUE-001: `cfg_attr(not(test), allow(dead_code))` does not survive `--all-targets`
**What happened:** Phase 1's three modules land before their callers, which
`-D warnings` rejects. The plan prescribed LESSON-016's remedy — a dated
`#[cfg_attr(not(test), allow(dead_code))]` — and it left 7 warnings standing.
**Root cause:** `just qc` lints with `cargo clippy --all-targets`, so the test
cfg is on and the attribute disables itself. It silences the binary build and
nothing else. The 24 non-test warnings and the 7 test warnings are two
separate problems with two separate remedies.
**Fix applied:** Kept the attribute for the non-test build *and* wrote tests
covering every remaining accessor. The tests were worth having anyway — one of
them asserts all nine path accessors, and a typo in any of them would have
sent a check looking for a file that does not exist and reported the corpus
clean.
**Recommendation:** When landing a module ahead of its callers, expect to do
both: the attribute for the binary build, and test coverage for the rest.
Treat the leftover warnings as a list of things the tests should be asserting,
not as noise to suppress.
**Skill:** none
**Gap:** answered — a cargo/clippy behaviour, learned by hitting it. The response is now folded into LESSON-016, which is where it belongs — a skill telling an agent about `--all-targets` would be a skill about Rust, not about this repo's process.

### ISSUE-002: phase 2's Done criteria could not both be satisfied
**What happened:** TEST-008 required `harness index --check` to exit 0 on the
current tree; TEST-012 required `just qc` green, and `qc` still ran
`scripts/ash.sh check`. Changing the generated file's attribution line to name
the new command satisfies neither: each generator then declares the other's
output stale.
**Root cause:** The plan treated the attribution change as cosmetic and
deferred only the *regeneration* to phase 5, not the rendered string. Two
implementations writing one file during a coexistence window cannot differ by
a single byte, and the plan's own trade-off note said so one paragraph away
from the criteria that contradicted it.
**Fix applied:** `GENERATOR` kept the bash spelling through phases 2–4 and
flipped in phase 5's TASK-041, which made that task real work instead of a
formality. Byte-identity became exact rather than modulo-a-line, which phases
3 and 4 needed anyway.
**Recommendation:** When a plan replaces something incrementally, list the
artifacts both implementations write and make "byte-identical until the
cutover" an explicit constraint of the coexistence phases. Then check the
per-phase Done criteria against it — mutual satisfiability is a property
`plan-write` Step 5b does not currently test for.
**Skill:** plan-write

### ISSUE-003: the plan-id scanner matched nothing, and only a side-by-side run showed it
**What happened:** `pinst harness check` reported all 23 recorded issues as
`learnings.untriaged` while `scripts/ash.sh check` reported a clean corpus.
**Root cause:** The scanner for `<yymmdd>-<six letters>` required a non-id
character after the 13-character match, to avoid matching a seven-letter hash.
But a lesson's `**Seen in:**` line cites plans by *directory* name, so the id
is nearly always followed by `-<slug>` — the boundary rule rejected every real
citation, the cited set came out empty, and every issue looked untriaged.
**Fix applied:** The trailing boundary now rejects a following lowercase
letter, which is what would extend the hash, and allows the slug hyphen.
**Recommendation:** This is the argument for the whole comparison harness.
Both implementations agreed on the healthy corpus and on five of six broken
fixtures; the disagreement only appeared because the real corpus was run
through both. When porting, diff against the incumbent on real data and on
deliberately broken data, not on a fixture you wrote to match your own mental
model.
**Skill:** none
**Gap:** answered — a coding bug in a scanner. No instruction prevents one; the comparison harness that caught it is the durable answer, and that is now LESSON-019.

### ISSUE-004: `lesson.unenforced` had to diverge from the script to be correct
**What happened:** The bash check grepped `scripts/` relative to the working
directory. Rooting the equivalent at the corpus means a fixture corpus in a
tempdir cannot resolve a check id that exists in the real repo, so the two
implementations disagree on any fixture with a mechanized lesson.
**Root cause:** The script assumed corpus and source were the same tree
because it only ever ran in this repo. Once the corpus can be another repo
(REQ-005), "relative to the working directory" and "relative to the corpus"
stop being the same thing.
**Fix applied:** Rooted at the corpus, and the shared fixtures use a check id
that exists nowhere, so both implementations agree. The real-corpus case is
covered separately by a test asserting every mechanized lesson resolves.
**Recommendation:** Deliberate divergences during a port are fine, but they
have to be named in the log at the moment they are made — otherwise the next
reader finds a comparison test carefully avoiding a case and cannot tell
whether that was judgement or an oversight.
**Skill:** none
**Gap:** answered — a design judgement made once, during one port. Nothing recurring for a skill to own.
**Distilled:** declined — a design note about this port, not a lesson that
would transfer. The general half is already ISSUE-006's.

### ISSUE-005: a 30-second constant made the test suite 30 seconds long
**What happened:** The test proving a hung `evidence:` command gets killed ran
for the full `TIMEOUT`, taking `cargo test` for the harness modules from 0.3s
to 30.05s.
**Root cause:** `evidence::run` read `TIMEOUT` internally, so a test could not
ask for a shorter one without changing a global.
**Fix applied:** The timeout became a parameter; callers pass `TIMEOUT`, the
test passes 300ms. 30.05s → 0.31s.
**Recommendation:** This is LESSON-018 recurring in a different disguise — not
an environment variable this time, just a constant read inside the function.
The tell is the same: the test is awkward because the value is ambient.
**Skill:** none
**Gap:** answered — already covered by LESSON-018, which is prose precisely because spotting ambient state is judgement.

### ISSUE-006: the check hard-coded this project's directory layout
**What happened:** `lesson.unenforced` searched `<corpus>/src` and
`<corpus>/scripts`. Running against a corpus copied into a bare directory —
the TEST-029 scenario — reported four spurious errors.
**Root cause:** `src` and `scripts` are pinst's layout, written into a command
whose whole premise (REQ-005) is that it runs against whatever repo the caller
is standing in. A repo mechanizing a lesson with a check in `tools/` or `lib/`
would have been told it did not exist.
**Fix applied:** Searches the repo root, skipping `.ash`, `.git` and `target`.
**Recommendation:** When a tool's scope widens from one repo to any repo, the
hard-coded paths are where the old assumption survives. Grep the new code for
this project's own directory names before calling the widening done.
**Skill:** none
**Gap:** answered — the plan's own Done criteria caught it. That is a plan-write practice already in use, not a missing skill.
**Distilled:** declined — the plan's own Done criteria caught this, which is
the system working rather than a gap in it. TEST-029 existed precisely to
exercise the scope-widening claim, and it did.

### ISSUE-007: a check that greps for a finding id was satisfied by a comment about it
**What happened:** Immediately after ISSUE-006's fix, `lesson.unenforced`
resolved `wire.missing` from a doc comment in `check.rs` — one I had just
written, explaining the search — rather than from `scripts/agents-wire.sh`.
Moving `agents-wire.sh` away left the check silent.
**Root cause:** The search is a plain substring match over source files, and
it cannot tell an id in a `finding()` call from an id in prose. Widening the
search from `scripts/` to the whole repo brought documentation into range, and
the first thing in range was a sentence naming a real id as an example.
**Fix applied:** Two things. The doc comment no longer contains a literal id
and says why. `.md`, `.txt` and `.log` are excluded from the search — an id
*emitted* by a check is in code; an id in documentation is someone writing
about the check. Verified by moving `agents-wire.sh` away and confirming
LESSON-007 fires, then restoring it and confirming it clears.
**Recommendation:** Any check that proves a marker exists by searching text
can be satisfied by the text describing the check. Test it the only way that
works: delete the thing it is supposed to find and confirm the check fires.
A passing check is not evidence until you have seen it fail.
**Skill:** none
**Gap:** answered — a subtle property of text-matching checks, now LESSON-021. Knowing it is the fix; no procedure would have surfaced it.

### ISSUE-008: a programmatic deletion truncated a file, and it still compiled
**What happened:** Removing the comparison oracle from `check.rs` with a
Python string-index slice cut from the oracle to the end of the module,
deleting six of eight tests. `cargo test` passed — with 2 tests instead of 8.
**Root cause:** The end marker was `"    }\n}\n"`, which matches the close of
the method *and* the close of the impl block and module. The first match was
the wrong one, and a truncated Rust file with balanced braces is still valid
Rust.
**Fix applied:** Restored from git and redid it with computed bounds plus
assertions on the lines being deleted. Caught by noticing the test count in
the output, not by any failure.
**Recommendation:** After deleting code programmatically, compare the test
count before and after — a green build says nothing about how much of the file
survived. Prefer bounds located by searching for a unique anchor and asserted
before the edit, over any slice from a first match.
**Skill:** none
**Gap:** answered — an editing mishap. LESSON-022 records the guard; a skill for 'how to delete code carefully' would be a skill nobody invokes.
