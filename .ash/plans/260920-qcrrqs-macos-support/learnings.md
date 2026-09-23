---
id: 260920-qcrrqs
slug: macos-support
updated: 2026-09-21
areas: [manifest, exec, release, configs]
issue_count: 8
---

# Learnings — macOS support: a released Darwin binary, and every command working behind it (260920-qcrrqs-macos-support)

## Source
- Plan: `./README.md`
- Basis: session context (an unattended overnight run implementing all five
  phases in one continuous session)
- Logs consulted: `./logs/20260920T202922Z-claude-sonnet-5.log` (own run,
  spanning an account-session-limit interruption and resume)

## Summary
All five phases landed: a `Platform` value threaded through selection,
probing, planning and doctor; a `brew` install method; every manifest tool
given a macOS answer; portable release tooling with Darwin matrix rows; and
`install.sh`/CI taught to recognize Darwin. `just qc` stayed green
throughout, and everything provable without a Mac was proven — including
exercising fallback branches (BSD tar, `shasum`, no-brew-on-PATH) with
stubs rather than assuming they worked. TASK-038 (confirming the whole path
on real hardware) stays open, per DEP-004: no Mac was available. The
headline lesson is that most of this plan's real bugs were not in the new
code Phase 1–2 wrote, but in *existing* functions that had never needed to
know a tool could resolve differently depending on context, and continued
reading the one field they'd always read. The macOS CI leg this plan itself
adds in Phase 5 then found one more of exactly that shape on its first real
run (ISSUE-008) — the one bug in this plan that `--platform macos`
simulation from Linux structurally could not have caught, which is the
whole reason Phase 5 exists rather than trusting simulation all the way
through.

## Issues

### ISSUE-001: Dependency ordering ignored platform-specific `requires` edges
**What happened:** `pinst plan --platform macos --json` ordered `homebrew`
*after* the three tools whose macOS override required it — `graph::topo_order`
sorted tools with no edges alphabetically, and `ripgrep`/`direnv`/`delta`'s
`requires = ["homebrew"]` lived only in their `[tool.platform.macos]`
override, invisible to a function that read `tool.requires` (the base field)
directly.
**Root cause:** `topo_order` predated platform overrides by two phases and
was never revisited when `Tool::resolve()` introduced a second,
platform-dependent view of a tool's edges. Every other consumer of an
`Install`/`Detect`/`post_install` field had already been rewritten to go
through `tool.resolve(platform)`; this one ordering function was missed
because it reads `requires`, a field that looks structural rather than
install-related.
**Fix applied:** added `topo_order_for(tools, platform)` sharing the same
Kahn's-algorithm core via a generic `topo_order_with(tools, requires_of)`
helper; `graph::select` now calls the platform-aware variant. `topo_order`
itself stays for `Manifest::validate`'s cycle check, which is deliberately
base-graph-only.
**Recommendation:** when adding a resolved/effective view over an entity
that already has several existing consumers, grep for every place the base
field is read directly — not just the fields the new work directly touches
(`install`, `detect`) but neighboring fields (`requires`) that a different,
older function also reads for an unrelated purpose (ordering, not planning).
**Skill:** none — this is a review discipline for a specific kind of
refactor, not a step a generic skill would have covered.
**Gap:** answered — this is domain-specific code review, not recurring work a
repository skill should own.

### ISSUE-002: A tool-level explicit upgrade override leaked across a platform's install substitution
**What happened:** `delta`'s macOS override replaces `install` (apt → brew)
but not `upgrade`. `upgrade_spec_for(MacOS)` returned `UpgradeSpec::Apt`
anyway — the code checked the override's own `upgrade` field, then fell
through unconditionally to the tool's base-level explicit `upgrade`
override, which was written for the apt install and had no idea a
different platform had replaced it.
**Root cause:** the fallback chain (override upgrade → base upgrade →
derived from effective install) treated "base upgrade override" as always
valid, when it is really scoped to "whichever install method the override
author had in mind" — which is the base install only when the platform
didn't substitute a different one.
**Fix applied:** the base-level explicit `upgrade` is now only inherited
when the platform override left `install` unset; if it replaced `install`,
resolution falls straight to deriving from the *effective* install.
**Recommendation:** any "explicit override" field that exists to correct a
*derivation* has to be re-scoped, not just re-checked, whenever the thing it
derives from becomes context-dependent. The bug wasn't in the derivation
logic (`derive_upgrade_spec`, unchanged and correct) — it was in a fallback
chain that let a stale correction survive a change to what it was
correcting.
**Skill:** none.
**Gap:** answered — this is a manifest-resolution edge case, not a missing
repository workflow skill.
**Distilled:** declined — narrow to this fallback-chain's specific shape (an override field correcting a derivation); LESSON-035 already covers the broader "old consumer of a base field" pattern this sits inside.

### ISSUE-003: A dependency cycle from copying the Linux `requires` pattern onto a platform-only tool
**What happened:** `homebrew`'s manifest entry carried `requires = ["curl"]`
(the pattern every other tool that needs curl follows). Once `curl` itself
gained a macOS override requiring `homebrew`, this became a two-node cycle,
caught immediately by `topo_order_for` at `pinst doctor` run time rather
than at manifest-parse time.
**Root cause:** the `requires = ["curl"]` edge was true on Linux (where curl
is a package pinst manages) and simply copied onto `homebrew`'s entry
without asking whether it was still true in the context `homebrew` actually
exists in — macOS, where curl ships with the OS and needs no pinst step
before Homebrew's own bootstrap runs.
**Fix applied:** removed the edge; documented why directly on the entry so
a future reader doesn't reintroduce it by the same copy-the-pattern
instinct.
**Recommendation:** this is exactly why LESSON-002 says to verify an
assumption with the real source rather than infer it — "curl needs to be
listed before things that need curl" is true as a general habit and false
for this specific tool on this specific platform. Caught fast here only
because `topo_order_for`'s cycle detection is a hard error, not a silent
misordering; a similar semantic bug (as opposed to a structural one) in a
`requires` edge would not get caught this way.
**Skill:** none — this is domain-specific to the manifest, not a skill gap.
**Gap:** answered — this is domain-specific manifest authoring, not recurring
work a repository skill should own.
**Distilled:** declined — a one-off manifest-authoring mistake (copying a Linux-true edge onto a platform where it wasn't), not a pattern likely to recur in a different shape.

### ISSUE-004: `curl_script`'s `args` field cannot express an env-var prefix
**What happened:** the phase 2 plan sketched Homebrew's install as
`curl_script` with `args = ["NONINTERACTIVE=1"]`. That would have produced
`bash -s -- NONINTERACTIVE=1` — a positional argument to the script, not an
environment variable — while Homebrew's installer reads `NONINTERACTIVE`
from the environment.
**Root cause:** `curl_script`'s `args` are documented (in the type itself)
as appended after `--`, which is correct for scripts that parse their own
flags, but Homebrew's installer is not that shape.
**Fix applied:** used the existing `shell` escape-hatch method instead:
`NONINTERACTIVE=1 /bin/bash -c "$(curl ...)"`, matching Homebrew's own
documented one-liner.
**Recommendation:** when a plan sketches an install method's fields for a
specific installer, check the installer's actual configuration mechanism
(env var vs. flag vs. positional arg) before committing to a method — this
is a narrower instance of LESSON-002's "verify against the real source
while planning," specific to install-method field shapes.
**Skill:** none.
**Gap:** answered — this is an installer-specific interface detail, not a
missing repository workflow skill.
**Distilled:** declined — specific to `curl_script`'s field semantics; the general form ("check the installer's actual config mechanism") is already LESSON-002.

### ISSUE-005: `just dist`'s cross-vs-native comparison had two ways to be wrong, not one
**What happened:** the original heuristic compared only the leading
architecture field (`x86_64` vs `x86_64`), which sent an arm64 Mac building
`x86_64-apple-darwin` down the `cross` branch. The first fix attempt
(comparing the *whole* remaining triple after the architecture) would have
broken the *existing*, working case: an `x86_64-unknown-linux-gnu` host
building `x86_64-unknown-linux-musl` differ in their trailing `gnu`/`musl`
field, so a whole-triple comparison would have wrongly sent that down
`cross` too — regressing LESSON-005 while fixing the macOS case.
**Root cause:** the right granularity is neither "just architecture" nor
"everything after architecture" — it's specifically vendor+OS. Getting this
from first principles required holding both the new failure mode (Darwin
arch mismatch) and the old one (Linux libc mismatch) in mind at once.
**Fix applied:** parse the triple into arch/vendor/os/abi and compare only
vendor+os between target and host.
**Recommendation:** when fixing a comparison heuristic that already has one
known-working case, write down that working case as a test (or at least a
manual check) *before* changing the comparison — not just the new case
motivating the fix. TEST-023 in this plan (byte-identical musl output) is
what caught this in review before it shipped; without it, the fix would
have looked complete on a Mac and silently broken the Linux release build.
**Skill:** none — but this generalizes; see LESSON promotion below.
**Gap:** answered — this is a domain-specific build heuristic, not recurring
work a repository skill should own.

### ISSUE-006: An "authored" docs page implicitly claims local verification
**What happened:** `docs/tools/homebrew.toml` was written with
`status = "authored"` and two `[[recipes]]`. `an_authored_page_with_recipes_says_which_version_it_was_verified_against`
failed: authored pages with recipes must name `verified_with`, and there
was no honest version to name — the recipes were written from Homebrew's
documented `brew info --json=v2` interface, never run.
**Root cause:** `status = "authored"` is not just "a human wrote this
carefully" — per this repo's own `PageStatus` doc comment, it specifically
means "every recipe in it was executed on this machine." Writing recipes
for a tool this session has no access to (no `brew` on Linux) and marking
them authored would have made a false claim the test exists to catch.
**Fix applied:** changed `status` to `draft`.
**Recommendation:** when writing a docs page for a tool the current
environment cannot run, default to `draft` from the start rather than
`authored` — the test that catches this is correct and worth listening to,
not worth routing around with a guessed `verified_with` value.
**Skill:** none — the `pinst` skill's existing docs-page guidance already
covers this if read carefully; this was not following it, not a gap in it.
**Gap:** answered — this is a docs contract and environment limitation, not a
missing repository workflow skill.
**Distilled:** declined — this is `PageStatus`'s existing, correct contract working as designed; nothing generalizes beyond "read the docs-page convention before authoring one".

### ISSUE-007: A plan's own task-level instruction conflicted with the supervising session's explicit override
**What happened:** TASK-038 (Phase 5) says in its own text "**do not mark
the phase Done without it**" — it needs a real Mac, which this run did not
have. The supervising agent's resume instructions separately said "TASK-038
... stays blocked-on-hardware; do not let it hold anything up," which is a
direct, task-named override of the plan's own instruction.
**Root cause:** the plan was written (correctly, at the time) assuming
implementation would have Mac access at some point in its lifecycle; the
actual run did not, and the human authority for this specific run had
already anticipated and pre-authorized that gap.
**Fix applied:** Phase 5 (and Phase 4, which has the same structure) marked
`Done` on the strength of their tree-provable done criteria, with the
hardware-gated criteria and TASK-038 itself left explicitly open and
un-fabricated in each phase's `## Confirmed after the merge` section. The
conflict and its resolution are logged in the run log rather than silently
resolved either way.
**Recommendation:** `plan-write`'s done-criteria split (LESSON-003) already
anticipates exactly this situation for the common case; what it does not
cover is a phase whose own task text goes further and explicitly forbids
marking the phase Done without external confirmation. When that happens and
a supervising authority has explicitly pre-authorized proceeding anyway,
record the conflict and the authorization by name — don't silently follow
either instruction without a trace of why.
**Skill:** none — this is a real conflict between two legitimate sources of
authority (the plan document and the session's actual operator), not
something a skill's instructions could have resolved in advance.
**Gap:** answered — this is an authorization conflict specific to one run, not
a missing repository workflow skill.
**Distilled:** declined — one occurrence, and the resolution (record the conflict and the authorization by name) is already stated as the recommendation; nothing further to mechanize.

### ISSUE-008: `pinst docs search`/`dump --tag` filtered out every tool the host couldn't install
**What happened:** PR #19's first real `macos-latest` CI run failed
`cli::commands::docs::tests::a_tag_filter_narrows_what_dump_and_search_can_see`
with `left: 0, right: 1` — invisible on Linux, where the same test passed
every time. The test's fixture manifest has four tools, all installed via
`apt` with no macOS override, so on macOS every one of them now resolves
`Unsupported` and is filtered out before the tag filter even runs.
**Root cause:** Phase 1 rewrote `docs.rs`'s local `select()` helper to go
through `graph::select(..., Platform::host())` for consistency with every
other command's narrowing. But `graph::select` also drops platform-
unsupported tools — the right behavior for a command about to *plan an
install*, and the wrong one for a command about to *look up
documentation*. `pinst docs show ripgrep` from a Linux box, or `docs dump
--tag dev` to see what a Mac's manifest documents, both want the whole
catalogue regardless of what this specific host could install today.
Nothing in `--platform macos` simulation from Linux could catch this: the
test fixture is entirely apt tools on *both* platforms in the sense that
matters — the bug only exists when `Platform::host()` genuinely resolves
to `MacOS`, which only a real macOS runner does.
**Fix applied:** rewrote `docs.rs`'s `select()` to narrow by tag/profile
directly over `manifest.tools`, with no platform involvement at all —
matching what it did before Phase 1 touched it. `graph::select` (platform
filtering, dependency closure, topological order) stays reserved for
commands that actually plan installs.
**Recommendation:** when a helper reuses another command's narrowing
function "for consistency", check whether that function's *other*
behaviors (not just the one being reused) apply to the new caller too.
`graph::select` bundles three things — tag/profile matching, platform
filtering, dependency ordering — and docs only wanted the first. This is
also the strongest evidence in this whole plan for why Phase 5's CI leg
exists at all: every other bug here (ISSUE-001 through ISSUE-004) was
caught by `--platform macos` simulation or by running the binary
directly, but this one specifically needed `Platform::host()` to actually
be `MacOS`, which only real hardware (or a real runner) provides.
**Skill:** none.
**Gap:** answered — this was a platform-specific implementation bug, not
recurring work a repository skill should own.
**Distilled:** promoted — LESSON-037.
