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
**Seen in:** 0001-release-please-binary-artifacts (ISSUE-002)

### LESSON-002: Check platform-side preconditions with an API call while planning
**Lesson:** Anything a plan assumes about GitHub, the runner image, or another
system's configuration should be verified with a real query before the plan is
written, not asserted as an ASSUMPTION for implementation to discover.
**Why:** These are one-command checks that cannot be inferred from the
codebase, and getting one wrong blocks a whole phase on someone else's action.
`gh api repos/<slug>/actions/permissions/workflow` would have shown up front
that this repo cannot let Actions open a pull request.
**Seen in:** 0001-release-please-binary-artifacts (ISSUE-001, ISSUE-007)

### LESSON-003: Split Done criteria that need a merge from those that do not
**Lesson:** When verification depends on landing on `main`, an admin action or
a deploy, write the phase's Done criteria in two halves — what is provable in
the working tree, and what can only be confirmed afterwards.
**Why:** Otherwise a phase whose code is complete and committed still reads as
unfinished, and the distinction between "not written" and "written, awaiting a
merge" is lost exactly when someone picks the work back up.
**Seen in:** 0001-release-please-binary-artifacts (ISSUE-005, ISSUE-008)

### LESSON-004: A manifest value that reaches a shell is read twice
**Lesson:** Manifest fields interpolated into shell commands (paths,
destinations) are interpreted once by Rust and once by `sh`. Before adding
Rust-side logic that inspects such a field, check whether the manifest writes
it unexpanded — `$HOME/...`, `${VAR:-default}`, `~/...` are all idiomatic here.
**Why:** `under_home()` compared `$HOME/.local/bin` as a literal path against
the expanded `$HOME`, concluded the destination was privileged, and would have
installed root-owned files into the user's own home.
**Seen in:** 0001-release-please-binary-artifacts (ISSUE-003)

### LESSON-005: `cross` is for foreign architectures, not a different libc
**Lesson:** Building x86_64 glibc → x86_64 musl needs only `musl-tools` and
(for `aws-lc-rs`) `cmake`; plain `cargo build --target ...` handles it. Save
`cross` and its container for a genuinely different architecture.
**Why:** It removes a Docker image pull from every release build and keeps the
same command working on a developer machine as in CI.
**Seen in:** 0001-release-please-binary-artifacts (ISSUE-004)

### LESSON-006: Print another action's outputs on the first run that uses them
**Lesson:** When a job branches on or consumes the outputs of a third-party
action, add a step that dumps `toJSON(steps.<id>.outputs)`, and read both the
prefixed and unprefixed spelling if the action has modes. Keep the step.
**Why:** An output name that does not exist evaluates to the empty string, so
the failure mode is a job that is silently skipped or runs with an empty
argument — indistinguishable in the UI from "nothing to do". release-please's
manifest mode emits `.--tag_name`, not `tag_name`, and the dump step is what
turned that from a broken release into a three-line fix.
**Seen in:** 0001-release-please-binary-artifacts (ISSUE-006)
