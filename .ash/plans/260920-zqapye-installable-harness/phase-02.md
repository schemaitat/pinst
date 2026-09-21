---
id: 260920-zqapye
slug: installable-harness
phase: 2
status: Done
---

# Phase 2 — `pinst harness install`

## Goal
Make the harness installable at either scope by flag — a plan of `Link`,
`Write` and `Backup` actions run through the same executor every other mutating
command uses — and record what was written, so the next phase has something
exact to undo.

## Why this phase exists
Phase 1 can read the world but not change it. This phase is the smallest slice
that changes it *usefully*: a complete non-interactive installer, driven by
flags, which is also the form an agent and CI will use. The interactive picker
(Phase 4) is a front end over these same flags, so building it first would mean
designing the selection UI before knowing what a selection consists of.

The receipt lands here rather than in Phase 3 because an install that does not
record itself produces an uninstall that has to guess — and guessing, in a
command that deletes, is the failure this plan most wants to avoid (SEC-001).

## Steps

- [x] TASK-010: verify ASSUMPTION-002 before building on it — install a skill
      and a command by hand into `~/.claude/skills/` and `~/.claude/commands/`,
      start a session, and confirm both load. Record the outcome in the run log
      either way.
      **Why:** this is the one thing in the plan pinst does not own, and it is
      one minute to check against a plan that would otherwise ship a global
      scope that quietly installs into a directory nobody reads (LESSON-002).
      If commands turn out not to be read globally, install skills only at
      global scope and note it here — nothing downstream depends on the answer.
- [x] TASK-011: `src/core/harness/install/receipt.rs` — `Receipt {
      schema_version, scope, vendor, source, style, installed_at,
      pinst_version, entries: Vec<Entry> }` where `Entry { kind, name, path,
      style }`; `path_for(scope)` returning `<root>/.ash/harness.json` for
      project and `${XDG_STATE_HOME:-~/.local/state}/pinst/harness.json` for
      global; `load` and `save`, with `#[serde(deny_unknown_fields)]`.
      **Why for the location:** one rule for both scopes would have put the
      global receipt at `~/.ash/harness.json`, and a bare `.ash/` at `$HOME`
      is found by `CorpusRoot::discover` from any directory outside a repo —
      `pinst harness check` would then report a corpus that does not exist as
      clean (ALT-002).
      **Why `deny_unknown_fields`:** it is hand-editable the moment someone
      debugs an install, and serde silently drops what it does not recognise
      (LESSON-017).
- [x] TASK-012: `src/core/harness/install/plan.rs` — `build_install_plan`, one
      `Step` per asset with id `harness:skill:<name>` / `harness:command:<name>`:
      `Skipped` when already in the desired state, `Blocked` on `Foreign` or
      `Unmanaged` without `--force`, `Backup` then `Link`/`Write` otherwise.
      Link style defaults to symlink when the source is a tree *and* the scope
      is project; copy otherwise.
      **Why the default flips by scope:** a link from `~/.claude/skills` into a
      git worktree dangles the moment that worktree is removed, and this repo
      is normally worked on through disposable worktrees (ALT-007).
- [x] TASK-013: `src/core/harness/install/corpus_init.rs` — create
      `.ash/{plans/,INDEX.md,LEARNINGS.md,CHANGELOG.log}` when the scope is
      project and `.ash/` is absent, as ordinary plan steps so `--dry-run`
      reports them; skipped entirely under `--no-corpus`, and never touching an
      existing file.
      **Why:** the skills are instructions for producing a corpus. Installing
      them into a repo with nowhere to write one leaves an agent to invent the
      layout, which is how the format drifts (REQ-007).
- [x] TASK-014: `src/cli/mod.rs` + `src/cli/commands/harness.rs` —
      `HarnessAction::Install(HarnessInstallArgs)` with `--scope`, `--vendor`,
      `--skill <name>` (repeatable), `--command <name>` (repeatable), `--all`,
      `--copy`, `--link`, `--force`, `--no-corpus`; execute via
      `commands::install::execute_and_report`, then write the receipt — and
      **only** when the run was not a dry run and nothing failed.
      **Why that ordering:** a receipt written before execution describes an
      intention, and uninstall would then try to remove paths that were never
      created.
- [x] TASK-015: `src/cli/mod.rs` + `src/cli/commands/schema.rs` — a
      `SchemaKind` variant emitting the receipt's JSON Schema, so the file an
      agent may have to read is documented by the same type that writes it.
- [x] TASK-016: tests — install into a tempdir project and assert the six links;
      a second run reports every step `Skipped`; `--dry-run` writes no file and
      no receipt; global scope from an embedded source produces copies; an
      existing real directory at a managed path comes back `Blocked` and
      untouched without `--force`, and `Backup`-then-write with it.

## Trade-offs & risks
SEC-001 is established here, not in Phase 3: uninstall can only be as safe as
the record install leaves. The `Blocked` path in TASK-012 is what keeps a
global install from touching skills the user wrote themselves (RISK-006) — it
reports them as `Unmanaged` and walks past, which is also what
`agents-wire.sh` does today and the one behaviour of it worth preserving
exactly.

`--force` exists and backs files up rather than deleting them, reusing
`Action::Backup` and its `.pre-pinst.<stamp>` naming. That is the same trade
`config apply` makes, and the same reason: a stock file at a managed path is a
situation, not a crime.

TASK-010 is a blocked-shaped task in the sense of LESSON-003 — it waits on a
human running a session — but it blocks nothing: the fallback (skills only at
global scope) is written down, so implementation proceeds either way rather
than deciding under pressure at the boundary.

## Done criteria
**Provable in the tree:**
- TEST-006: `cargo test` covers install, re-install, dry-run, copy mode, and the
  blocked-on-unmanaged path.
- TEST-007: `pinst harness install --scope project --all --dry-run` in a clean
  tempdir prints every step and creates nothing, including no receipt.
- TEST-008: `pinst harness install --scope project --all` in that tempdir
  creates `.claude/skills`, `.claude/commands`, `.ash/`, and a receipt at
  `.ash/harness.json`; `pinst harness status` then reports everything `Linked`.
- TEST-009: `pinst schema` emits the receipt schema and it contains `entries`.

**Confirmed by hand (TASK-010, needs a live session):**
- TEST-010: a skill and a command installed at global scope are actually loaded
  by Claude Code, or the phase records which of the two is not and narrows the
  global scope accordingly.
