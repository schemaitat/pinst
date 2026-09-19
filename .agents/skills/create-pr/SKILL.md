---
name: create-pr
description: Open a pull request for the current branch, building its title and body from the plan the work was authorized by. Use whenever the user asks to open, create, raise, or put up a PR or pull request, to "ship this branch", or to get work reviewed — even if they don't name the skill. Also use it to fix up a PR description that has drifted from what the branch actually does.
---

# Create PR

Opens a pull request for the current branch. The important idea: **the PR
description has already been written.** If this branch implements a plan, that
plan's ADR holds the context, the decision, the alternatives that were weighed
and the consequences — recorded when the reasoning was fresh, not
reconstructed from a diff afterwards. This skill assembles a PR from it rather
than paraphrasing the changes.

A branch with no plan behind it still gets a PR; it just gets one built from
the commits instead, and that is the honest signal that the change was small
enough not to need a plan.

______________________________________________________________________

## Step 1 — Preflight

CI (`.github/workflows/ci.yml`) runs exactly what `just qc` runs, so a red
`qc` is a red PR. Check, in this order, and stop on anything that fails:

```sh
git branch --show-current          # must not be main
git status --short                 # must be empty — commit first
just qc                            # 0, or fix before opening
git log --oneline main..HEAD       # must be non-empty
```

- **Not on `main`.** If the work is sitting on `main`, stop and say so; the
  fix is a branch, and that is the user's call, not a silent `git checkout -b`.
- **Nothing uncommitted.** Use the `conventional-commits` skill for anything
  outstanding. A PR that omits working-tree changes misrepresents the branch.
- **`just qc` green.** Never open a PR you already know CI will fail. If it
  fails, fix it or tell the user what is broken.
- **Behind `main`?** `git rev-list --count HEAD..main` — if non-zero, merge
  `main` in first so the PR shows the real diff and CI tests the real result.

## Step 2 — Find the plan

`plan-implement` puts a `Plan: <id>-<slug>` footer on every commit it makes,
which is how a branch says what authorized it:

```sh
git log main..HEAD --format=%B | grep '^Plan: ' | sort -u
```

- **Exactly one plan** — the normal case. Read
  `.ash/plans/<id>-<slug>/README.md` in full, plus every `phase-NN.md` whose
  frontmatter `status` is `Done`. These are the source for Step 4.
- **No plan** — a small change that never needed one. Build the body from
  `git log main..HEAD` instead and keep it short; do not invent an ADR after
  the fact.
- **More than one plan** — the branch is doing more than one thing. Say so and
  propose splitting it before opening anything. Plans are the unit of
  reviewable work here; a PR spanning two of them is a PR nobody can review
  against its own stated intent.

Also note which phases this branch covers, and which of the plan's phases it
does **not** — a partially implemented plan is normal and belongs in the body.

## Step 3 — The title

**The title must be a valid Conventional Commit.** Use the
`conventional-commits` skill for the type and scope, exactly as for a commit:

```
feat(agents): wire skills into .claude and add harness checks
```

This is not a style preference. `main` is released by release-please, which
reads Conventional Commits. Merge-commit and rebase merges preserve the
branch's own commits, so release-please still sees them — but **squash merge
is enabled on this repo**, and under a squash the PR title *becomes* the
commit subject on `main`. A title release-please cannot parse produces no
version bump and no changelog entry, silently: the work ships and never
appears in a release.

So: **do not prefix the title with the plan id.** The `plan-write` skill says
to (`[260919-qwerty] Add MLflow…`), and PR #1 did exactly that — it survived
only because that PR was not squashed. The id belongs in the body, where no
tool parses it. Flag this to the user if they ask for the bracket form.

If the branch is one phase of an unfinished plan, say so in the title's scope
or subject rather than with a prefix, e.g.
`feat(engine): add the dependency graph (phase 2 of 5)`.

## Step 4 — The body

Assemble from the plan's ADR. Every section below maps to one the plan already
has — copy the substance, tighten the prose, do not re-derive it:

```markdown
## What this does
<the plan's ## Decision, tightened to 2–4 sentences>

## Why
<the plan's ## Context — the problem and why it needed solving now>

## Alternatives considered
<the plan's ## Alternatives Considered table, verbatim>

## Phases
| # | Phase | Status |
|---|-------|--------|
| 1 | <title> | Done — in this PR |
| 2 | <title> | Not started |

## Consequences
<from the plan's ## Consequences and the per-phase ## Trade-offs & risks:
what becomes harder, what was deferred, what a reviewer should watch>

## Verification
<the Done criteria that actually passed, and `just qc` — say which>

Plan: <id>-<slug>
```

Drop any section the plan leaves empty rather than writing a placeholder. For
a no-plan PR, keep `## What this does`, `## Why` and `## Verification`, and
omit the rest.

**`## Alternatives considered` is the section reviewers benefit from most and
the one a diff can never supply** — it pre-empts "why not just…?" with the
answer that was already worked out. Never omit it when the plan has one.

Mark the PR a draft (`--draft`) when the plan has phases still unstarted: it
is a checkpoint, not a finished argument.

## Step 5 — Open it

Push the branch first if it has no upstream. Pushing is the point of no return
— the branch becomes visible — so if it has never been pushed, say what you
are about to push and let the user stop you.

```sh
git push -u origin "$(git branch --show-current)"
gh pr create --title "<title>" --body-file <file> [--draft]
```

Write the body to a file and pass `--body-file` rather than `--body`; a body
with backticks, `$`, or newlines does not survive shell quoting intact.

Never `--fill` (it would overwrite the body you just built from the plan), and
never target anything but the default branch unless asked.

## Step 6 — Report

Give the user the PR URL, the title as it landed, whether it is a draft, and
which phases it covers. Then check CI once:

```sh
gh pr checks --watch
```

If CI fails on something `just qc` passed, that gap is itself worth reporting
— the two are supposed to be identical, and a divergence is a bug in the
justfile or the workflow, not a flaky test to re-run.

## Step 7 — Afterwards

Opening the PR does not finish the plan. If this branch completed the plan's
last phase and `plan-learnings` has not run yet, run it now — the reasoning is
still in context, and it will not be after review.
