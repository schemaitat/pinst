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
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-002)

### LESSON-002: Check platform-side preconditions with an API call while planning
**Lesson:** Anything a plan assumes about GitHub, the runner image, or another
system's configuration should be verified with a real query before the plan is
written, not asserted as an ASSUMPTION for implementation to discover.
**Why:** These are one-command checks that cannot be inferred from the
codebase, and getting one wrong blocks a whole phase on someone else's action.
`gh api repos/<slug>/actions/permissions/workflow` would have shown up front
that this repo cannot let Actions open a pull request.
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-001, ISSUE-007)

### LESSON-003: Split Done criteria that need a merge from those that do not
**Lesson:** When verification depends on landing on `main`, an admin action or
a deploy, write the phase's Done criteria in two halves — what is provable in
the working tree, and what can only be confirmed afterwards.
**Why:** Otherwise a phase whose code is complete and committed still reads as
unfinished, and the distinction between "not written" and "written, awaiting a
merge" is lost exactly when someone picks the work back up.
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
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-003)

### LESSON-005: `cross` is for foreign architectures, not a different libc
**Lesson:** Building x86_64 glibc → x86_64 musl needs only `musl-tools` and
(for `aws-lc-rs`) `cmake`; plain `cargo build --target ...` handles it. Save
`cross` and its container for a genuinely different architecture.
**Why:** It removes a Docker image pull from every release build and keeps the
same command working on a developer machine as in CI.
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
**Seen in:** the harness audit on `feat/agents-orchestration` (both legacy
plans predated the format their own skills mandate); again in
260919-zeuuaj-release-please-binary-artifacts (ISSUE-009).

### LESSON-009: Internal consistency is not freshness — check the record against something written after the fact
**Lesson:** A checker that compares a record only with itself will pass on a
record that describes a world which no longer exists. Whenever work is
handed off to someone else — a merge, an admin action, a deploy — the last
item in the handover is returning the outcome to the record, and something
mechanical has to notice when that did not happen. Point the check at
whatever local artifact is written *after* the fact; in this repo that is the
append-only `.ash/CHANGELOG.log`, which is why `ash.sh check` now compares it
against every phase's status.
**Why:** The corpus reported `corpus clean (3 plans)` for a full day while it
told its next reader to merge two already-merged PRs and to expect a version
three releases old. Every invariant held; all of them were about the corpus
agreeing with itself. The contradiction was sitting in plain sight — the
changelog recorded phases 3 and 4 as shipped while the phase files said
`In Progress` — and nothing was comparing the two halves. Staleness costs
more than inconsistency, because a stale record still reads as authoritative.
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
**Seen in:** 260919-zeuuaj-release-please-binary-artifacts (ISSUE-010)
