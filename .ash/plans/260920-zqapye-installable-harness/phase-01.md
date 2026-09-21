---
id: 260920-zqapye
slug: installable-harness
phase: 1
status: Done
---

# Phase 1 — `.agents/` becomes the whole shippable harness

## Goal
Make `.agents/` the single source of truth for everything an agent runtime
loads — skills *and* slash commands — embed it into the binary, and land
`pinst harness status` as the first thing that reads it, so a machine with no
clone can already answer "where is the harness installed here".

## Why this phase exists
Nothing can be installed before there is one thing to install. Today the
harness is split across two directories with different provenance: skills live
in `.agents/skills/` and are projected, commands live in `.claude/commands/`
and are not projected at all because they were authored in the vendor
directory. An installer built on that split would have two source rules and
would embed a vendor path inside the binary (ALT-004).

It also lands a *reader* rather than only a data structure. `just qc` lints
with `-D warnings` and `--all-targets`, so a module with no consumer is a build
failure dressed as a design choice — and of the three APIs written ahead of
their callers in `260920-tensvp`, one was deleted and one was wrong
(LESSON-016). `status` is the natural first consumer: it needs the source, the
vendor table, the root discovery and the state classifier, and it needs no
receipt, because a state is computed by comparing a target against the source
rather than by remembering what was done.

## Steps

- [ ] TASK-001: `.claude/commands/*.md` → `.agents/commands/*.md` — `git mv` all
      six (`cc`, `distil`, `implement`, `learn`, `plan`, `pr`), then create
      `.claude/commands/<name>.md` as a relative symlink to
      `../../.agents/commands/<name>.md` and commit both halves together.
      **Why:** the move and the projection have to be one commit. Between them
      this repo's own `/plan` and `/implement` do not exist, and the session
      that notices is the one trying to use them (RISK-002).
- [ ] TASK-002: `build.rs` — add `cargo:rerun-if-changed=.agents`.
      **Why:** cargo cannot see through `include_dir!`, so without this an
      edited skill changes nothing cargo considers an input and the previous
      build's copy keeps shipping — invisible on a dev machine, which reads the
      tree directly, and visible only on the machine running the download
      (LESSON-015).
- [ ] TASK-003: `src/core/harness/asset.rs` — `static EMBEDDED: Dir<'_> =
      include_dir!("$CARGO_MANIFEST_DIR/.agents")`, and
      `resolve_source() -> Source` delegating to
      `core::source::resolve(".agents", "PINST_HARNESS_SOURCE")` so the harness
      tree, `configs/` and `docs/tools/` cannot disagree about what a checkout
      is (PAT-002).
- [ ] TASK-004: `src/core/harness/asset.rs` — `AssetKind::{Skill, Command}`,
      `Asset { kind, name, files: Vec<PathBuf> }`, and
      `enumerate(&Source) -> Result<Vec<Asset>>` walking both `skills/*/` and
      `commands/*.md` from either a tree or the embedded `Dir`. Enumeration is
      recursive within a skill directory.
      **Why:** a skill is a directory, not a file — references and helper
      scripts belong to it, and a projector assuming one `SKILL.md` per skill
      truncates the first skill that has more (RISK-005).
- [ ] TASK-005: `src/core/harness/vendor.rs` — `Vendor::Claude`, `Vendor::ALL`,
      and a method giving the skills and commands subdirectory for a scope
      (`.claude/skills`, `.claude/commands`, and `~/.claude/...`). One table,
      one place to add a runtime (REQ-008, DEP-003).
- [ ] TASK-006: `src/core/harness/project.rs` — `InstallRoot::resolve(explicit)`:
      an explicit `--root`, else the nearest ancestor holding `.ash/`, else the
      git top-level, else the working directory.
      **Why:** deliberately not `CorpusRoot`, which requires a `.ash/` to
      exist. A repo adopting the harness has none yet — that is the whole point
      of installing — so reusing corpus discovery would make the command refuse
      to run exactly where it is most useful (REQ-007).
- [ ] TASK-007: `src/core/harness/install/state.rs` — `AssetState { Linked,
      Copied, Missing, Drifted, Foreign, Unmanaged }` and `classify` /
      `status(source, vendor, scope)`, mirroring `configs::FileState` and
      `ConfigSet::classify` including the dangling-symlink handling.
      **Why:** an operator reading `pinst config status` and `pinst harness
      status` should not have to learn two vocabularies for the same six
      situations (PAT-002, GUD-001).
- [ ] TASK-008: `src/cli/mod.rs` + `src/cli/commands/harness.rs` —
      `HarnessAction::Status` with `--scope project|global|both` (default
      `both`) and `--vendor` (default `claude`); human mode prints a banner per
      scope and a table of assets, `--json` puts the rows in `items` and the
      per-scope counts in `summary`. Status exits `3` when anything is
      `Missing`, `Drifted` or `Foreign`, and `0` otherwise (CON-001).
- [ ] TASK-009: tests — the embedded tree matches the checkout tree file for
      file (the `docs` catalogue test's shape, which is what caught LESSON-015);
      enumeration finds six skills and six commands; each `AssetState` variant
      is produced from a constructed `tempdir`; the status envelope puts rows in
      `items` and counts in `summary`.
      **Why:** the envelope assertion is not ceremony — `260920-wtburh` shipped
      a `status: issues` envelope whose findings appeared nowhere in it, caught
      only when a consumer tried to read it (LESSON-008, ISSUE-013).

## Trade-offs & risks
RISK-002 is the one that bites within the hour, and TASK-001 is written to make
it impossible rather than unlikely: one commit, both halves.

RISK-004 is accepted here and not revisited: `.agents/` goes into the binary,
`.ash/` does not. The line is whether the content is something pinst *delivers
to a machine* or something *the checked repo owns*, which is the same line
`260920-wtburh` drew.

DEP-001 was verified during planning rather than assumed — a scratch crate
confirmed `include_dir!` embeds a dot-prefixed directory and keys entries
relative to it — so TASK-003 should be a transcription, not a discovery. If it
is not, that is worth an ISSUE, because it means the verification was wrong
(LESSON-002).

The recursive enumeration in TASK-004 is speculative in the sense that no
current skill has a second file. It is written anyway because the cost is a
`walk()` that already exists in `configs.rs` and the failure mode is silent
truncation.

## Done criteria
- TEST-001: `just qc` is green with the commands moved; `/plan` and `/cc` still
  resolve through `.claude/commands/` symlinks.
- TEST-002: `cargo test` proves the embedded `.agents/` tree is byte-identical
  to the checkout tree.
- TEST-003: `pinst harness status` in this repo reports project scope fully
  `Linked` (six skills, six commands) and exits `0`; with one link removed it
  reports `Missing` and exits `3`.
- TEST-004: `pinst harness status --json | jq '.items[0].state'` returns a
  state string, and `.summary.project.linked` is a number.
- TEST-005: `PINST_HARNESS_SOURCE` pointed at a tempdir tree is honoured, and a
  binary run from outside any checkout falls back to the embedded copy.
