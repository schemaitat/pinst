# Distilled learnings

Lessons that generalize beyond the plan that produced them. Read this
before planning new work.

### LESSON-001: Reproduce a failure before writing code to prevent it
**Lesson:** When a plan proposes a change whose only justification is "X would
fail otherwise", reproduce X first. If it does not reproduce, drop the change
rather than keeping it as insurance.
**Why:** Mitigations have their own cost — here it would have been a
`sudo rm -rf` in the install path of a tool built around never clobbering
anything. A risk reasoned from a general rule ("the kernel refuses writes to a
running executable") can be defeated by a detail of the specific tool involved
(GNU tar unlinks before extracting), and the only way to find that out is to
try it.
**Status:** prose
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-002)

### LESSON-002: Check platform-side preconditions with an API call while planning
**Lesson:** Anything a plan assumes about GitHub, the runner image, or another
system's configuration should be verified with a real query before the plan is
written, not asserted as an ASSUMPTION for implementation to discover.
**Why:** These are one-command checks that cannot be inferred from the
codebase, and getting one wrong blocks a whole phase on someone else's action.
`gh api repos/<slug>/actions/permissions/workflow` would have shown up front
that this repo cannot let Actions open a pull request.
**Status:** prose
**Mechanize:** declined — no check can tell whether a plan verified its
assumptions or merely sounded confident; the evidence is the query, and the
query happens before anything a script can see.
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-001,
ISSUE-007); again in 260919-vldfei-self-improving-agent-harness (ISSUE-002,
ISSUE-003), where the unchecked assumptions were about the repo's own scripts
and its own corpus — closer to hand than GitHub, and for that reason trusted
without a query; and again in 260920-tensvp-tool-docs-explorer (ISSUE-002,
ISSUE-006), where the plan asserted which of 26 local binaries answer
`--help` without running any of them, and named a verification method no
dependency in the tree can perform. Both were one loop away from being facts.

### LESSON-003: Split Done criteria that need a merge from those that do not
**Lesson:** When verification depends on landing on `main`, an admin action or
a deploy, write the phase's Done criteria in two halves — what is provable in
the working tree, and what can only be confirmed afterwards.
**Why:** Otherwise a phase whose code is complete and committed still reads as
unfinished, and the distinction between "not written" and "written, awaiting a
merge" is lost exactly when someone picks the work back up.
**Status:** prose
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-005,
ISSUE-008, and again as ISSUE-009 — the lesson was written during that plan
and still not applied to its own phases, which is what let them go stale)

### LESSON-004: A manifest value that reaches a shell is read twice
**Lesson:** Manifest fields interpolated into shell commands (paths,
destinations) are interpreted once by Rust and once by `sh`. Before adding
Rust-side logic that inspects such a field, check whether the manifest writes
it unexpanded — `$HOME/...`, `${VAR:-default}`, `~/...` are all idiomatic here.
**Why:** `under_home()` compared `$HOME/.local/bin` as a literal path against
the expanded `$HOME`, concluded the destination was privileged, and would have
installed root-owned files into the user's own home.
**Status:** prose
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-003)

### LESSON-005: `cross` is for foreign architectures, not a different libc
**Lesson:** Building x86_64 glibc → x86_64 musl needs only `musl-tools` and
(for `aws-lc-rs`) `cmake`; plain `cargo build --target ...` handles it. Save
`cross` and its container for a genuinely different architecture.
**Why:** It removes a Docker image pull from every release build and keeps the
same command working on a developer machine as in CI.
**Status:** prose
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-004)

### LESSON-006: Print another action's outputs on the first run that uses them
**Lesson:** When a job branches on or consumes the outputs of a third-party
action, add a step that dumps `toJSON(steps.<id>.outputs)`, and read both the
prefixed and unprefixed spelling if the action has modes. Keep the step.
**Why:** An output name that does not exist evaluates to the empty string, so
the failure mode is a job that is silently skipped or runs with an empty
argument — indistinguishable in the UI from "nothing to do". release-please's
manifest mode emits `.--tag_name`, not `tag_name`, and the dump step is what
turned that from a broken release into a three-line fix.
**Status:** prose
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-006)

### LESSON-007: A skill that is not projected into the runtime's own directory never loads
**Lesson:** After adding or renaming anything under `.agents/skills/`, run
`just wire`, and let `just harness` verify it. Treat "the file exists in the
repo" as saying nothing about whether an agent can actually load it.
**Why:** `.agents/` is a vendor-neutral convention, not a path any runtime
reads — Claude Code reads `.claude/skills/`. An unwired skill fails silently:
there is no error, the skill simply never appears, and the nearest thing that
*does* load is whatever stale copy exists in someone's personal `~/.claude`.
Silence is the whole problem, which is why the check has to be mechanical.
**Status:** mechanized
**Check:** wire.missing
**Seen in:** the harness audit on `feat/agents-orchestration` — all four repo
skills were unwired, and a drifted personal copy of `plan-write` was loading
in their place.

### LESSON-008: An invariant that lives only in prose has already drifted
**Lesson:** When a convention is worth writing into a skill, write the check
that enforces it in the same change, and wire the check into `just qc`.
**Why:** Skills are instructions to a model that may or may not be in context
when the relevant edit happens, and they say nothing at all about the
documents written before the convention existed. Every invariant the plan
skills stated in prose — quoted indices, no duplicate `## Status` section,
a regenerated `INDEX.md` — was being violated by the corpus at the moment the
skills were committed. An exit code is checkable by anyone, at any time,
without having read the skill.
**Status:** prose
**Mechanize:** declined — this is the rule that produces checks, not a rule a
check can express. `lesson.unenforced` mechanizes its narrow half: a claim of
enforcement must name something real.
**Seen in:** the harness audit on `feat/agents-orchestration` (both legacy
plans predated the format their own skills mandate); again in
260919-zeuuaj-release-please-binary-artifacts (ISSUE-009); again in
260919-vldfei-self-improving-agent-harness (ISSUE-004), where six SKILL.md
files had never been valid YAML because only hand-written parsers had ever
read them; and again in 260920-tensvp-tool-docs-explorer (ISSUE-008), where a
Done criterion narrowed during implementation and became a test in the same
change rather than a corrected sentence in the plan.

### LESSON-009: Internal consistency is not freshness — check the record against something written after the fact
**Lesson:** A checker that compares a record only with itself will pass on a
record that describes a world which no longer exists. Whenever work is
handed off to someone else — a merge, an admin action, a deploy — the last
item in the handover is returning the outcome to the record, and something
mechanical has to notice when that did not happen. Point the check at
whatever local artifact is written *after* the fact; in this repo that is the
append-only `.ash/CHANGELOG.log`, which is why `pinst harness check` now compares it
against every phase's status.
**Why:** The corpus reported `corpus clean (3 plans)` for a full day while it
told its next reader to merge two already-merged PRs and to expect a version
three releases old. Every invariant held; all of them were about the corpus
agreeing with itself. The contradiction was sitting in plain sight — the
changelog recorded phases 3 and 4 as shipped while the phase files said
`In Progress` — and nothing was comparing the two halves. Staleness costs
more than inconsistency, because a stale record still reads as authoritative.
**Status:** mechanized
**Check:** phase.logged-not-done
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-009)

### LESSON-010: Never run a state-deriving tool inside the window where the workflow hides that state
**Lesson:** When a workflow deliberately keeps something provisional for part
of its run — a draft release, an unpushed tag, an unmerged PR — no tool that
derives its state from that same thing may run before it becomes real. Split
the tool's invocation in two and gate the second half on the thing being
published.
**Why:** A draft GitHub release has no git tag, and a tagged release is how
release-please knows what has already shipped. Doing both jobs in one
invocation — create the draft, then compute the next release PR — made the
second half re-read the history from the first commit and propose another
version bump, whose merge created another draft, which blinded the next run.
Two releases and an empty third release PR came out of that loop in fifteen
minutes, each changelog a copy of the whole history. The tell is generic
enough to reuse: a release PR listing entries older than the previous release
has lost its boundary, whatever version it proposes.
**Status:** prose
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-010)

### LESSON-011: A check that fires on a normal state is a check that gets switched off
**Lesson:** Before adding a finding, name the states in which it fires and
confirm none of them is a state the workflow *requires*. If the thing it
reports is a mandatory intermediate step, gate it on the terminal event that
ends that step rather than on the absence of the final artifact.
**Why:** `plan.learnings-missing` fires from the moment a run writes its first
log line, and `learnings.md` cannot exist until the run ends — so `just qc` is
red for the entire duration of every implementation run, by design, on every
plan. A gate that is always red during work is a gate people learn to skip,
and then it is not a gate. The fix is the same shape as the cure: compare
against the record written after the fact, here a `run_end` event in the log.
**Status:** prose
**Seen in:** 260919-vldfei-self-improving-agent-harness (ISSUE-001)

### LESSON-012: Grade a tool by the artifact it leaves, not by whether it ran
**Lesson:** When measuring whether some piece of automation is working, make
it declare the artifact it is supposed to produce and check that artifact.
Invocation counts are a second opinion at best: they are vendor-specific,
they under-count every alternative entry point, and they move when you look
at them.
**Why:** Counting invocations would have graded the busiest lifecycle skill in
this repo as dead — `plan-implement` recorded zero, from either record shape,
while having written a complete run log. Slash commands log differently from
tool calls, so `/cc` and `/pr` were invisible too. And the count is reflexive:
implementing the measurement bumped `plan-implement` from 0 to 1. A commit
that parses, a plan that carries its ADR sections, a run log with a matching
`run_start` and `run_end` — these are durable, in-repo, vendor-neutral, and
none of them changes because you read it.
**Status:** mechanized
**Check:** skill.no-contract
**Seen in:** 260919-vldfei-self-improving-agent-harness (ISSUE-005)

### LESSON-013: Make the corpus the clock, not the calendar
**Lesson:** When work needs doing "regularly", find the event in the record
that should trigger it and fail on that, instead of scheduling it. A finding
that fires when there is something to do beats a job that fires on Tuesdays.
**Why:** A wall-clock cadence fires into silence on a quiet week and misses
four plans on a busy one, and it lives in one person's account rather than in
the repo, so a fresh clone does not inherit it. `learnings.untriaged` fires
exactly when a plan has recorded something nobody has decided about, stays
until someone decides, and everyone who runs `qc` sees it. The trigger is a
plan closing, which is the thing that actually creates the work.
**Status:** mechanized
**Check:** learnings.untriaged
**Seen in:** 260919-vldfei-self-improving-agent-harness

### LESSON-014: A report that asks a question needs somewhere to record the answer
**Lesson:** Add the "asked and answered" mechanism in the same change that
adds the question. A recurring report with no way to retire an item will keep
raising it after it has been settled.
**Why:** The gap section asks whether a skill is missing for work no skill
owns. The first review answered it — no, those four issues are domain
surprises — and there was nowhere to put the answer, so it recomputes and asks
again every time. Issues got three outcomes including an explicit decline;
gap groups got none, and the omission reproduced, inside this plan's own last
phase, exactly the decay this plan was written to prevent: a signal that is
correct, repeated, and eventually ignored.
**Status:** prose
**Seen in:** 260919-vldfei-self-improving-agent-harness (ISSUE-006)

### LESSON-015: A macro that reads files at compile time must declare them as build inputs
**Lesson:** When a macro embeds a directory or file into the binary
(`include_dir!`, `include_str!` over generated content), add a `build.rs`
emitting `cargo:rerun-if-changed` for those paths in the same change. Treat
"the data is in the repo" as saying nothing about whether the compiled
artifact contains it.
**Why:** Cargo cannot see through a macro, so files read during expansion are
not tracked inputs: edit only data files and nothing is recompiled, and the
previous build's embedded copy keeps shipping. The failure is invisible where
you would notice it and visible where you cannot debug it — a dev machine
reads the source tree directly and looks correct, while a machine running the
downloaded binary silently serves a stale copy. In this repo it had been
latent since `configs/` was first embedded, and only surfaced because a test
compared the checkout tree against the embedded catalogue, written for an
unrelated reason.
**Status:** prose
**Seen in:** 260920-tensvp-tool-docs-explorer (ISSUE-004)

### LESSON-016: Add an API in the phase that consumes it
**Lesson:** In a phased plan, do not land a function, a method or a module
before the code that calls it. Where a module genuinely has to arrive first,
make the gap loud and self-deleting — an `#[cfg_attr(not(test),
allow(dead_code))]` whose comment names the phase that removes it — rather
than a quiet allow.
**Why:** Phase boundaries are slices of design, but `just qc` enforces a
property of the whole tree at every commit, and `-D warnings` makes dead code
a build failure. Speculative API also tends to be wrong: of three items
written ahead of their callers here, one was deleted and re-added with a
different signature, one turned out to be unnecessary, and one only earned its
place by finding a use nobody had planned (reporting orphan pages).
**Status:** prose
**Mechanize:** declined — the mechanical half already exists and is what
forces the issue: `clippy --all-targets -D warnings` is exactly the build
failure this lesson is about surviving. What the lesson adds is the judgement
of *how to respond* — omit the API, or mark it and date the mark — and no
exit code can distinguish a well-placed allow from a lazy one.
**Seen in:** 260920-tensvp-tool-docs-explorer (ISSUE-003); again in
260920-wtburh-harness-in-the-binary (ISSUE-001), which showed the remedy is
only half of one: `just qc` lints with `--all-targets`, so the test cfg is on
and the attribute disables itself. The binary build and the test build are two
separate warning sets — the attribute settles the first, and the second has to
be settled by tests that actually exercise the new API.

### LESSON-017: Reject unknown fields in any format a human writes by hand
**Lesson:** Put `#[serde(deny_unknown_fields)]` on hand-authored config types.
The cost is one attribute; the alternative is a file that loads clean and
means something other than what it says.
**Why:** Serde ignores unknown fields by default, so a typo, a
singular/plural slip, or a key that landed in the wrong TOML table silently
disappears — and the result is not an error but a document that lies. The
first page authored in this repo put a top-level key below an
array-of-tables, where TOML binds it to that table; the guard turned a page
that would have quietly lost its content into a parse failure naming the
field.
**Status:** prose
**Seen in:** 260920-tensvp-tool-docs-explorer (ISSUE-001)

### LESSON-018: A test that wants to override ambient state is telling you to make it a parameter
**Lesson:** When isolating a test means setting an environment variable, a
global, or a process-wide default, change the code to take the value as an
argument instead of building machinery around the environment.
**Why:** Process-wide state forces a lock, the lock has to be held for the
duration of the call, and `clippy::await_holding_lock` rejects holding one
across an `.await` — so the workaround does not even work in async code. The
parameter version is shorter, needs no lock, and removes the possibility of a
test reaching the developer's real `~/.cache` at all. The awkward test was a
design problem one level up, not a testing problem.
**Status:** prose
**Mechanize:** declined — the trigger is "this test feels awkward", which is
not a property of the source. A check could flag `set_var` in tests, but both
sightings so far were something else: a process-wide default in one, a plain
`const` read inside a function in the other. The pattern is recognisable and
not detectable.
**Seen in:** 260920-tensvp-tool-docs-explorer (ISSUE-005); again in
260920-wtburh-harness-in-the-binary (ISSUE-005), where it was not an
environment variable but a plain `const` read inside the function — a
30-second timeout that made the test proving it works take 30 seconds. Same
tell: the test is awkward because the value is ambient.

### LESSON-019: Compare a port against the incumbent on broken input, not on healthy input
**Lesson:** When reimplementing something that already works, keep both
versions runnable and diff them on deliberately broken input for as long as
the old one exists. Agreement on the happy path is nearly free and proves
nearly nothing. Write the comparison to be *deleted* with the incumbent, not
adapted.
**Why:** Both implementations of the corpus checker agreed on the real corpus
and on five of six broken fixtures. The sixth disagreement was a scanner for
plan ids that required a non-id character after the match — but a lesson cites
plans by directory name, so every real citation is followed by `-<slug>`, the
scanner matched nothing, and all 23 recorded issues came out as "untriaged"
against a bash run of "clean". Nothing in the healthy corpus could have shown
that, because the bug's whole effect was to make a set come out empty, and an
empty set is what a clean corpus looks like.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-003)

### LESSON-020: Two implementations that write one file must agree byte for byte
**Lesson:** During a changeover where old and new both generate the same
artifact, treat byte-identical output as a hard constraint of every phase
before the cutover — attribution lines, comments and all. Change the bytes in
the commit that deletes the loser, and check the per-phase Done criteria are
mutually satisfiable before writing them.
**Why:** Each generator checks whether the file on disk matches what it would
produce, so a one-character difference has each declaring the other's output
stale — and one of them runs inside `qc`. Phase 2 of this plan wrote two
criteria that could not both hold: `harness index --check` exits 0, and
`just qc` (still running the script) stays green. The plan's own trade-off
note said byte-identity was the constraint, one paragraph from the criteria
that broke it, so this is not something more care at writing time would catch
— it needs to be a question asked of the finished criteria.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-002)

### LESSON-021: A check that searches text can be satisfied by the text describing it
**Lesson:** Any check that proves something exists by grepping the tree can be
satisfied by documentation *about* that check. Exclude prose from the search,
and prove the check works the only way that counts: delete what it is supposed
to find and watch it fire.
**Why:** `lesson.unenforced` confirms a lesson's claimed enforcement by
searching the source for the finding id it names. Widening that search from
one directory to the whole repo brought doc comments into range, and the first
thing in range was a comment — written in the same change — using a real
finding id as an example. The lesson resolved against that sentence rather
than against the script that emits it, and deleting the script left the check
silent. A passing check is not evidence until it has been seen to fail.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-007)

### LESSON-022: A deletion that still compiles has not been verified
**Lesson:** After removing code programmatically, compare the test count
before and after. Locate the bounds with a unique anchor and assert on the
lines about to go, rather than slicing from the first match of a generic
pattern.
**Why:** Cutting one test helper out of a Rust file with a string-index slice
whose end marker was `"    }\n}\n"` matched the close of the enclosing impl
block instead of the method, deleting six of the file's eight tests. The
result compiled, `cargo test` passed, and the only visible symptom was the
count dropping from 8 to 2 in output nobody has to read. Balanced braces make
a truncated Rust file indistinguishable from an intact one to the compiler,
which is exactly the class of edit where "it builds" is worth nothing.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-008)

### LESSON-023: A sweep filtered by file type only finds the callers you remembered
**Lesson:** When deleting or renaming something other files invoke, grep the
whole tree with no `--include` filter and read every hit. Write the *check*
for leftovers the same way — a Done criterion that greps `*.md` and the
justfile is a criterion that passes while CI is broken.
**Why:** Retiring `scripts/ash.sh` swept the justfile, both plan skills, two
slash commands and both READMEs, and left `.github/workflows/ci.yml` calling
it — because the phase's own verification step filtered on `*.md` and
`justfile`. The filter was written from the same memory that did the sweep, so
it could only confirm what had already been thought of. CI found it two pushes
later, which is the cheapest place it could still have gone wrong and the most
annoying place to notice.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-010)

### LESSON-024: CI that restates the gate instead of calling it will drift from it
**Lesson:** When a workflow duplicates the steps of a local quality gate, that
duplication is an unenforced claim that the two are identical. Either call the
gate (`just qc`) or accept that every change to it is a two-file change, and
say so in the file where the copy lives.
**Why:** `ci.yml` opens by stating that its steps mirror `qc` "so a green CI
and a green `just qc` mean the same thing", and then restates them by hand to
avoid depending on `just`. That is a reasonable trade, but it made the claim
false the moment the justfile's harness recipe changed, and nothing local
could detect it: `just qc` was green on the very commit CI could not run. The
same shape as LESSON-008 — an invariant asserted in prose — except the second
copy here is executable, which makes it look maintained.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-010)

### LESSON-025: A documented degradation is a test specification
**Lesson:** When a plan writes down that something degrades in a hostile
environment — no network, no auth, a missing binary — that sentence is
telling you what a test must arrange or avoid depending on. Reproduce the
hostile case locally by shadowing the binary with a stub that exits non-zero;
it is two lines and it catches the class before CI does.
**Why:** This plan's DEP-003 said the `create-pr` conformance measure
"degrades to `unmeasured` when it is absent or the network is" — and a test
was then written asserting the real corpus produces no findings *while
running that measure*. It passed locally only because `gh` happened to be
authenticated, and failed on the first CI run. The risk was not unforeseen;
it was foreseen, written down, and then not carried into the test that
depended on it, which is the more common failure and the harder one to
notice.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-009)

### LESSON-026: A write to an external system is verified by reading it back
**Lesson:** Never pipe a command through `tail` or `head` when its exit code
is what you are checking — `$?` becomes the pipe's last stage and is almost
always 0. Capture the output to a file, or test `${PIPESTATUS[0]}`. And for
anything that writes to a system you do not control — a PR body, a release, a
remote file — the exit code is not the check anyway: fetch it back and assert
on what is actually there.
**Why:** `gh pr edit --body-file` failed against this repo because it fetches
project cards as part of its update and GitHub now rejects that query — a
deprecation with nothing to do with the body being written. It reports the
failure properly, exiting **1**. But the command was run as `... 2>&1 | tail
-2`, so the exit code belonged to `tail`, the error line read like a routine
deprecation notice, and the PR body silently stayed as it was. What caught it
was fetching the body back and finding the new section absent; what would have
caught it sooner was not discarding the status of the thing being checked. The
same pipe habit was used on `just qc` throughout that session and got away
with it only because its failures happen to print a recognisable line near the
end.
**Status:** prose
**Seen in:** 260920-wtburh-harness-in-the-binary (ISSUE-011)
