---
id: 260920-tensvp
slug: tool-docs-explorer
updated: 2026-09-20
areas: [cli, docs, manifest, tui]
issue_count: 8
---

# Learnings — A baked-in, searchable catalogue of tool usage (260920-tensvp-tool-docs-explorer)

## Source
- Plan: `./README.md`
- Basis: session context, cross-checked against this run's log
- Logs consulted: `logs/20260920T090304Z-claude-opus-5.log`

## Summary
All six phases completed in one run: the page format, the read path, capture
and adopt, search and dump, 26 authored pages, and the Docs tab — 142 tests,
`just qc` green throughout. The headline lesson is ISSUE-004, which the plan
did not anticipate and which had been latent in this repo since `configs/` was
first embedded: `include_dir!` reads files at compile time that cargo never
learns about, so adding 25 pages changed nothing cargo considered an input and
the binary kept shipping the one-page catalogue. It was caught only because
phase 1 had written a test comparing the checkout tree against the embedded
copy — a test written for a different reason entirely.

The other recurring shape was smaller and appeared three times: work
sequenced by phase kept colliding with `just qc`'s zero-warning rule, because
a module written in phase N has no consumer until phase N+1 (ISSUE-003).

## Issues

### ISSUE-001: A bare key after `[[recipes]]` became a field of the recipe
**What happened:** The first authored page put `gotchas` below its
`[[recipes]]` tables. TOML bound it to the last recipe rather than to the
page, and the parse failed with "unknown field `gotchas`, expected `cmd` or
`does`".
**Root cause:** TOML array-of-tables scoping. Everything after `[[x]]` belongs
to that table until the next header — which is invisible when the file reads
top-to-bottom like prose.
**Fix applied:** Every top-level key moved above the first `[[recipes]]`
table, and the rule written into the page itself, into the seed `docs adopt`
writes, and into the SKILL.md authoring section.
**Recommendation:** Put `#[serde(deny_unknown_fields)]` on any format humans
write by hand. Without it this file would have loaded clean with `gotchas`
silently attached to a recipe and no gotchas on the page — a page that lies
rather than a page that fails. The same guard later caught a `[[recipe]]`
singular/plural typo in a test.
**Skill:** none
**Gap:** answered — this work is not skill-shaped: a property of TOML, caught by a serde attribute; no instruction file would have prevented it.

### ISSUE-002: No JSON Schema validator to validate the schema against
**What happened:** TEST-002 asked that `pinst schema docs` emit a schema which
validates the authored page. Nothing in the dependency tree can validate a
document against a JSON Schema.
**Root cause:** The plan wrote the criterion in terms of the outcome
("validates") without checking whether the toolchain could express it, and
CON-003 forbids adding a dependency to a size-tuned binary.
**Fix applied:** The test asserts every top-level key of the real page appears
in the emitted `properties`, and that `required` is exactly `["what"]`. That
catches the drift that actually matters — a field the schema does not know
about — without a validator.
**Recommendation:** When a Done criterion names a verification method, check
that the method exists in this repo's dependency set while the plan is still
being written. The weaker check was fine; discovering the need for it during
implementation is what cost time.
**Skill:** plan-write

### ISSUE-003: A phase boundary that lands code with no consumer fights the no-warnings gate
**What happened:** Three times. Phase 1 left the whole `core::docs` module
unreachable from the binary until the CLI landed in phase 2, so
`clippy -D warnings` failed on dead code. Phase 2 had the same problem with
`Catalogue::page_path`, written for an `adopt` that did not exist yet. Phase 1
also tripped it on a single unused `as_str`.
**Root cause:** The plan's phase boundaries are vertical slices of *design*
(format, then read, then capture) but `just qc` enforces a property of the
whole tree at every commit. A module and its first caller can land in
different phases; a compiled binary has no way to express "not yet".
**Fix applied:** Different answers for different causes, and the difference is
the point. The module-wide gap was real and temporary, so it got an explicit
`#[cfg_attr(not(test), allow(dead_code))]` with a comment naming the phase
that removes it — deleted one commit later. `page_path` was API written
speculatively, so it was deleted and re-added in phase 3 with `adopt` as its
consumer. `iter` earned its place immediately by reporting orphan pages, which
turned out to be a feature worth having.
**Recommendation:** Add an API in the phase that consumes it. Where a whole
module genuinely has to land before its caller, make the gap loud and
self-deleting rather than quiet — an `allow` that names the phase which
removes it cannot outlive its purpose silently.
**Skill:** plan-write

### ISSUE-004: `include_dir!` hides its inputs from cargo, so the embedded copy went stale
**What happened:** After writing 25 pages, the test comparing the checkout
tree against the embedded catalogue failed: the tree had 26 pages, the binary
had 1. Nothing was wrong with either loader.
**Root cause:** `include_dir!` reads the directory during macro expansion.
Cargo does not see through the macro, so no file under `docs/tools/` (or
`configs/`) was a tracked build input. Editing only data files changed nothing
cargo considered a reason to recompile, and the previous build's embedded copy
kept shipping.
**Fix applied:** `build.rs` emitting `cargo:rerun-if-changed` for `configs`,
`docs/tools` and `manifest.toml`.
**Recommendation:** Any macro that reads files at compile time needs a
`build.rs` declaring those files as inputs, added in the same change as the
macro. Note the failure mode this had before the fix: on a dev machine the
source tree is read directly, so everything looks right; the stale copy only
surfaces on a machine running the downloaded binary, which is the worst place
to find it and the hardest to reproduce. This had been latent since `configs/`
was first embedded.
**Skill:** none
**Gap:** answered — this work is not skill-shaped: a property of cargo's build graph, now fixed in the repo and recorded as LESSON-015; nothing about the way this work is done would have changed it.

### ISSUE-005: Isolating a test from `$XDG_CACHE_HOME` needed a lock clippy rejects
**What happened:** Tests that exercised the capture cache set `XDG_CACHE_HOME`
to a temp directory, guarded by a process-wide mutex because `set_var` is
global. `clippy::await_holding_lock` failed the build: the guard was held
across every `.await` in an async test.
**Root cause:** The cache location was ambient state read deep inside the
call, so the only place to override it was the environment — which is global,
which is why the lock existed at all.
**Fix applied:** The cache directory became a parameter, resolved once in
`run()` and threaded down. No env var, no lock, no clippy failure, and no test
can reach the developer's real `~/.cache` even by accident.
**Recommendation:** When a test wants to isolate something the code reads from
the environment, that is the signal to make it a parameter rather than to
build machinery around the environment. The lock was a workaround for a design
problem one level up.
**Skill:** none
**Gap:** answered — this work is not skill-shaped: a clippy rule meeting a design smell, answered by the design change and recorded as LESSON-018.

### ISSUE-006: The plan guessed which tools needed a `help_cmd` override, and guessed wrong
**What happened:** The plan assumed several tools would need an override for
`<bin> --help`, and named `build-essential` among the tools with no CLI at
all. Running the help command for all 26 showed that every tool with a
`detect.bin` answers `<bin> --help` unmodified, and `build-essential` answers
through gcc.
**Root cause:** The list was reasoned from the manifest rather than measured
by running anything — the plan asserted a property of 26 binaries from their
declarations.
**Fix applied:** One loop over the manifest, running each candidate command.
The real overrides are four and all structural: `nvm` is a shell function that
must be sourced, `git-credential-manager` has a CLI but two possible names and
therefore no `detect.bin`, and the two zsh pieces have no command at all and
now say so with an empty `help_cmd`.
**Recommendation:** This is LESSON-001 in a new costume — the measurement was
a single loop and would have taken a minute during planning. When a plan says
"the tools whose X is not Y", enumerate them by running the check, not by
reading the manifest.
**Skill:** plan-write

### ISSUE-007: Two ranking tests asserted the wrong thing, and the code was right
**What happened:** `a_keyword_outranks_prose` searched for "grep" and expected
a keyword match on ripgrep's page; it got a name match, because *ripgrep*
contains *grep*. `a_query_of_nothing_but_stopwords_still_tries` asserted no
fixture tool matched "it", but a recipe's text contains "file it would
search".
**Root cause:** Both expectations were written from the intent of the rule
rather than from the data the fixture actually contained, and substring
matching on short terms hits far more than it looks like it will.
**Fix applied:** Both expectations corrected to describe the real behaviour
and split into separately named tests, so the name-beats-keyword rule is
asserted deliberately instead of by accident.
**Recommendation:** For a ranking function, write the test against the fixture
data, not against the rule you had in mind — and prefer fixture strings that
cannot accidentally satisfy a neighbouring rule. A failing test here was the
system working: it found two wrong beliefs about the ranking before the
catalogue was authored against them.
**Distilled:** declined — "write the test against the fixture, not against the
rule you meant" is ordinary test-writing care, specific to this ranking
function's substring matching; a general lesson would not have prevented it.
**Skill:** none
**Gap:** answered — this work is not skill-shaped: ordinary test-writing care about fixture data; already declined for distillation.

### ISSUE-008: A Done criterion had to be refined once the content existed
**What happened:** TEST-010 required every `authored` page to carry
`verified_with`. Two pages — oh-my-zsh and zsh-autosuggestions — describe
things with no command and no version at all, so there was nothing to pin,
and marking them `draft` would have implied their content was unverified.
**Root cause:** The criterion was written while the catalogue was a format,
before the actual content showed that a page with no recipes is a legitimate
shape.
**Fix applied:** The rule was narrowed to what it was really claiming — an
authored page *with recipes* must name the version they were verified against
— and mechanized as a test over the embedded catalogue rather than left as
prose in the plan.
**Recommendation:** When implementation narrows a criterion, write the
narrowed rule as a check in the same change (LESSON-008), and record the
narrowing rather than quietly satisfying the original wording.
**Skill:** plan-implement
