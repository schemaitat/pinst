---
name: conventional-commits
description: 'Craft and make git commits in this repo (pinst) using the Conventional Commits format (type(scope): subject). Use this whenever the user asks to commit changes, write a commit message, "commit this", "make a commit", or wrap up a change with a commit — even if they don''t say "conventional commits" explicitly. Also use it to validate or fix up a commit message someone already wrote. This repo''s entire history follows this convention, so every new commit should too.'
produces: 'every commit in this repo''s history parses as a Conventional Commit'
evidence: 'echo $(git log --format=%s $ASH_RANGE | grep -cE ''^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?!?: '') $(git log --format=%s $ASH_RANGE | grep -c .)'
---

# Conventional Commits for pinst

This repo's history is 100% Conventional Commits already (`feat:`, `fix(configs):`,
`chore:`, `docs:`, `style:` ...). Every new commit should keep that pattern intact
so `git log` stays skimmable by type/scope and a changelog could be generated from
it mechanically later.

The general git safety rules (stage specific files, never `-A`/`.`, new commit over
amend, no force-push, attribution footer) still apply as normal — this skill is
specifically about getting the *message* right.

## The format

```
type(scope)!: subject

optional body, wrapped ~72 chars, explaining *why* not *what*

optional footer(s): BREAKING CHANGE: ..., Refs #123, Closes #123
```

- `scope` is optional — omit it for changes that don't belong to one area.
- `!` right after the type/scope (before `:`) marks a breaking change; pair it
  with a `BREAKING CHANGE:` footer that explains what breaks.

## Choosing a type

| type | when |
|------|------|
| `feat` | a new capability or behavior |
| `fix` | a bug fix |
| `docs` | documentation only, no code |
| `style` | formatting/whitespace, no logic change (e.g. `rustfmt`) |
| `refactor` | code restructuring with no behavior change |
| `perf` | a performance improvement |
| `test` | adding or correcting tests only |
| `build` | build system, packaging, or dependency changes |
| `ci` | CI configuration |
| `chore` | maintenance that doesn't touch src or tests (tooling, justfile, etc.) |
| `revert` | reverting a prior commit — reference its hash/subject in the body |

When a change is a mix (e.g. a fix that also updates docs), pick the type for
the *primary* intent of the commit rather than trying to encode both — if the
two parts are genuinely unrelated, that's usually a sign they belong in
separate commits.

## Choosing a scope

A scope names the area touched — a config package, a module, a subsystem —
not the whole repo. Before inventing a new one, check what this repo already
uses:

```sh
git log --format=%s -50 | grep -oE '\([a-zA-Z0-9_.,/ -]+\)' | sort -u
```

Reuse an existing scope (`configs`, `docs`, ...) when the change fits one of
them. Only introduce a new scope when the change is clearly its own area and
you expect to touch it again — a scope used exactly once is often a sign the
commit didn't need one.

## Subject line rules

- Imperative mood: "add", "fix", "point" — not "added"/"adds"/"pointed".
- Lowercase right after the colon.
- No trailing period.
- Aim for ~50 characters, treat ~72 as a hard ceiling.

## Body and footers

Only add a body when the diff alone doesn't explain *why* the change was
made — a one-line diff rarely needs one, a workaround for a subtle bug
usually does. Skip footers entirely unless there's a breaking change or an
issue to reference.

If this commit implements work tracked under `.ash/plans/<id>-<slug>/`
(see the `plan-write` skill), add a `Plan: <id>-<slug>` footer line too,
after any other footers — that's how the plan's permanent id gets carried
into `git log`.

## Workflow

1. See what's actually changing: `git status`, then `git diff` for unstaged
   work and `git diff --cached` for anything already staged.
2. If nothing is staged, stage the specific files this commit is about (never
   `git add -A` or `git add .`) — re-check `git status` afterward to confirm
   nothing unrelated got swept in.
3. If the staged changes clearly cover more than one unrelated concern,
   consider proposing separate commits rather than forcing one message to
   cover both.
4. Check the scope vocabulary (command above) before picking a type/scope
   pair.
5. Draft the header, and a body/footer only if warranted.
6. Commit via a heredoc so multi-line messages format correctly:
   ```sh
   git commit -m "$(cat <<'EOF'
   type(scope): subject

   optional body
   EOF
   )"
   ```
7. Confirm it landed: `git log -1 --format='%H %s'` or `git status`.

If the commit touches `.ash/` or `.agents/`, run `just harness` before
step 6 — a commit that leaves `.ash/INDEX.md` stale or a skill unwired
passes review easily and is annoying to track down later.

## Examples (drawn from this repo's own history)

```
fix(configs): point .zshenv's uv PATH entry at $HOME
chore: add just install to put the binary on PATH
fix(docs): correct arrowhead rendering in the diagrams
docs: add architecture and workflow diagrams
feat: make pinst a self-contained, manifest-driven toolchain CLI
style: rustfmt the tree
```
