---
id: 260920-zqapye
slug: installable-harness
status: In Progress
created: 2026-09-20
updated: 2026-09-21
areas: [harness, agents, cli, tui]
summary: Make the agent harness something pinst installs, uninstalls and reports on — project or global, interactively or by flag — and show in both the CLI and the TUI where it is installed right now.
files_touched: [.agents/commands/cc.md, .agents/commands/distil.md, .agents/commands/implement.md, .agents/commands/learn.md, .agents/commands/plan.md, .agents/commands/pr.md, .agents/README.md, build.rs, src/core/harness/asset.rs, src/core/harness/vendor.rs, src/core/harness/project.rs, src/core/harness/install/mod.rs, src/core/harness/install/state.rs, src/core/harness/install/plan.rs, src/core/harness/install/receipt.rs, src/core/harness/install/check.rs, src/core/harness/install/corpus_init.rs, src/core/harness/mod.rs, src/core/harness/root.rs, src/core/harness/skills.rs, src/core/plan.rs, src/core/exec/mod.rs, src/cli/mod.rs, src/cli/commands/harness.rs, src/cli/commands/schema.rs, src/prompt.rs, src/app.rs, src/ui/mod.rs, src/ui/harness.rs, src/ui/statusbar.rs, src/event.rs, justfile, scripts/agents-wire.sh, scripts/distil-guard.sh, .github/workflows/ci.yml, .github/workflows/distil.yml, README.md]
---

# The harness becomes something pinst installs

## Context
The harness is two directories and a shell script. `.agents/skills/` holds six
vendor-neutral skills; `.claude/skills/` holds six committed relative symlinks
pointing back at them; `.claude/commands/` holds six slash-command files that
are not projected from anywhere because they were authored in the vendor
directory to begin with. `scripts/agents-wire.sh` writes the symlinks and
verifies them, and `just qc` runs its `--check`.

That arrangement works for exactly one repo: this one. It has no answer for a
machine that installed the release binary and wants the harness in `~/.claude`,
no answer for a second repo that wants to adopt the lifecycle, and no way to
remove what it put down. There is also nowhere to look up the one question an
operator actually asks — *is the harness installed here, and from what?*
`.claude/skills` being a directory of symlinks is the only evidence, and it is
evidence you have to know how to read (REQ-001, REQ-003, REQ-004, REQ-006).

Plan `260920-wtburh` moved the corpus *checks* into the binary for this reason
and stopped one step short: it left projection in bash, recording the gap as
RISK-006 — "the harness needs no `scripts/`" was true of the corpus half and
not of the recipe. Meanwhile `pinst` already solves this exact problem for
`configs/`: one source tree, embedded at compile time, symlinked into the place
the consumer looks when a checkout exists and materialized as real files when
it does not (PAT-002). The harness is the same shape of problem and has been
solved differently only because it started as a script (REQ-008, CON-004).

## Decision
Add `pinst harness install | uninstall | status`, implemented under
`src/core/harness/install/` with the rendering in `src/cli/commands/harness.rs`
and a fifth TUI tab (PAT-001, PAT-003). `.agents/` becomes the single source of
truth for everything an agent runtime loads — the six slash commands move from
`.claude/commands/` into `.agents/commands/` — and the whole directory is
embedded with `include_dir!`, so a binary downloaded onto a machine with no
clone carries the harness it installs (REQ-006, DEP-001).

**Installation is a `core::plan::Plan`, like everything else that mutates**
(CON-002). One step per asset, built from `Action::Link` when a checkout is
present and `Action::Write` when the source is embedded, executed through the
same `execute_and_report` that `config apply` uses — so `--dry-run`, the
authorization gate, the JSON envelope and the exit codes come for free and
cannot drift from a real run (CON-001).

**Two scopes, one receipt each.** Project scope writes into `<repo>/.claude/`
and records what it wrote at `<repo>/.ash/harness.json`, beside the corpus the
harness produces. Global scope writes into `~/.claude/` and records at
`${XDG_STATE_HOME:-~/.local/state}/pinst/harness.json` — deliberately *not*
`~/.ash/`, because a bare `.ash/` directory at `$HOME` would be found by
`CorpusRoot::discover` from any directory that is not inside a repo, and
`pinst harness check` would confidently report that phantom corpus as clean
(ALT-002). The receipt is what makes uninstall exact: it removes the paths it
recorded and nothing else, and a real file that someone put at a managed path
is reported rather than deleted (SEC-001, REQ-003).

**`scripts/agents-wire.sh` is retired in the same plan.** Keeping it beside a
Rust projector would be two implementations of one invariant, which is
precisely the drift this harness exists to detect and it would be detecting it
about itself. Every finding id it emits — the `wire.*` family and the
`skill.*` source validations — is carried over **verbatim as a string literal**
in Rust, because `LESSON-007` names `wire.missing` in its `**Check:**` line and
`lesson.unenforced` greps `src/` for that literal (CON-005, GUD-002).

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: keep `agents-wire.sh` and ship install beside it | Two implementations of one invariant, in two languages, with `just qc` running both. `260920-wtburh` already rejected this shape for the corpus checks; the argument does not change because the subject is symlinks |
| ALT-002: put the global receipt at `~/.ash/harness.json` | One rule for both scopes, but it plants a `.ash/` at `$HOME` that `CorpusRoot::discover` finds from any non-repo directory. `pinst harness check` run in `~/Downloads` would report a clean corpus that does not exist — a wrong answer delivered confidently, which is the failure mode `root.rs` was written to avoid |
| ALT-003: put the global receipt inside `~/.claude/` | Self-locating, and it dies with the directory it describes. But it scatters pinst's state into a vendor-owned folder, and every runtime added later needs its own answer to where state lives |
| ALT-004: leave the slash commands in `.claude/commands/` | The embedded tree would then contain a vendor directory, and a second runtime means a second copy of every command rather than one more projection target. The move costs six `git mv`s once |
| ALT-005: require a checkout; do not embed `.agents/` | Kills the main use case. `pinst` installs from a GitHub release onto a machine with no clone, which is the machine most likely to want a global harness |
| ALT-006: add `dialoguer` or `inquire` for the interactive prompt | The release profile is tuned for size (`opt-level = "z"`, LTO, `strip`), and `ratatui` + `crossterm` are already linked in for the TUI. A ~120-line selector over primitives already paid for beats a dependency that duplicates them (CON-003) |
| ALT-007: symlink global installs into a checkout by default | A link from `~/.claude/skills` into a git worktree dangles the moment the worktree is removed, and this repo is normally worked on through disposable worktrees. Global copies, `--link` to override |
| ALT-008: store a content hash per entry in the receipt | Adds a hashing dependency to answer a question the source can already answer: the embedded or checked-out `.agents/` is always available, so drift is a comparison against it, exactly as `ConfigSet::classify` does |
| ALT-009: a top-level `pinst install-harness` command | The `harness` family already exists and already owns this subject. A second top-level verb would split it |
| ALT-010: make install mutate the filesystem directly, without a `Plan` | Every other mutating path in pinst builds a plan first, which is what makes `--dry-run` honest. A second style of mutation is a second place for dry-run to be wrong |

## Consequences

What gets easier: any repo can adopt this lifecycle with one command, and
`.ash/` is scaffolded for it (REQ-007). The question "where is my harness"
becomes one command and one TUI tab rather than an `ls -la` you have to
interpret (REQ-004, REQ-005). `just wire` becomes a thin alias over the binary,
and `scripts/` loses its last harness responsibility.

What gets harder: `Action::Remove` is the first destructive action pinst has
ever had, and it lands in a tool whose entire design is about never clobbering
anything (RISK-001). It is constrained three ways — only receipt-recorded
paths, blocked on drift unless `--force`, and visible in `--dry-run` like every
other action — but the constraint is now load-bearing rather than structural,
which is a real change in this codebase's safety story.

- **DEP-001**: `include_dir` 0.7.4, already a dependency. Verified against a
  scratch crate during planning: `include_dir!("$CARGO_MANIFEST_DIR/.agents")`
  embeds a dot-prefixed directory and keys its entries relative to it
  (`skills/foo/SKILL.md`, not `.agents/skills/foo/SKILL.md`). This resolves
  what would otherwise have been an assumption (LESSON-002).
- **DEP-002**: `ratatui` and `crossterm`, already dependencies, carry the
  interactive picker as well as the new tab.
- **DEP-003**: Claude Code's directory layout — `.claude/skills/`,
  `.claude/commands/` and their `~/.claude/` equivalents. This is the one thing
  here pinst does not own; it is confined to `vendor.rs` so a change costs one
  table (REQ-008).
- **RISK-001**: `Action::Remove` — see above. Mitigated in Phase 3 and tested
  there.
- **RISK-002**: moving the slash commands out of `.claude/commands/` unwires
  this repo's own `/plan`, `/implement` and `/cc` if the projection does not
  land in the same commit. Phase 1 writes the relative symlinks by hand
  alongside the move, so the repo is never in an unwired state; Phase 2's
  installer then reproduces exactly those links.
- **RISK-003**: `lesson.unenforced` can be satisfied by its own subject matter.
  `260920-wtburh` hit this: a doc comment containing the literal `wire.missing`
  made LESSON-007 resolve against prose rather than against a check. Phase 6
  refers to the ids by shape in prose and proves the real thing by moving the
  module aside and confirming the finding fires.
- **RISK-004**: embedding `.agents/` ships this repo's own six skills inside
  every binary. Accepted, and it is the same trade as `configs/`: the skills
  are content pinst delivers to a machine. `.ash/` stays out, because a plan
  corpus belongs to whichever repo the caller is standing in — `260920-wtburh`
  decided that and nothing here reopens it (CON-004).
- **RISK-005**: a skill directory may hold more than `SKILL.md` — references,
  scripts, assets. Enumeration is recursive and copy mode copies the tree; a
  projector that assumed one file per skill would silently truncate skills that
  have not been written yet.
- **RISK-006**: a global install writes into a directory that already holds the
  user's personal skills. Anything not in the receipt is `unmanaged` and is
  reported and left alone, forever (SEC-001).
- **ASSUMPTION-001**: Claude Code follows a symlinked skill *directory*. Held
  true in this repo since `agents-wire.sh` was written, and `--copy` already
  exists as the escape hatch; it becomes `--copy` on the new command.
- **ASSUMPTION-002**: `~/.claude/skills/` and `~/.claude/commands/` are read
  for global scope the way their project equivalents are. Verified at the top
  of Phase 2 by installing there and loading a skill, before anything depends
  on it.
- **ASSUMPTION-003**: no runtime other than Claude Code needs to be supported
  for this plan to be useful. Stated by the user; `vendor.rs` is the seam that
  keeps it cheap to be wrong.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | `.agents/` becomes the whole shippable harness | [phase-01.md](phase-01.md) | Done |
| 2 | `pinst harness install` | [phase-02.md](phase-02.md) | Done |
| 3 | `pinst harness uninstall` | [phase-03.md](phase-03.md) | Done |
| 4 | The interactive picker | [phase-04.md](phase-04.md) | Proposed |
| 5 | The Harness tab | [phase-05.md](phase-05.md) | Proposed |
| 6 | Retire `scripts/agents-wire.sh` | [phase-06.md](phase-06.md) | Proposed |

## Affected Files
- **FILE-001**: `.claude/commands/*.md` → `.agents/commands/*.md` — six slash
  commands move to the vendor-neutral tree; `.claude/commands/` becomes six
  relative symlinks, committed, exactly as `.claude/skills/` already is.
- **FILE-002**: `build.rs` — `cargo:rerun-if-changed=.agents`, so editing a
  skill invalidates the embedded copy (LESSON-015).
- **FILE-003**: `src/core/harness/asset.rs` (new) — the embedded `.agents/`
  tree, source resolution, and recursive asset enumeration.
- **FILE-004**: `src/core/harness/vendor.rs` (new) — `Vendor::Claude` and the
  directory table; the single place a second runtime is added.
- **FILE-005**: `src/core/harness/project.rs` (new) — install-root discovery
  (`.ash/`, else git top-level, else cwd), distinct from `CorpusRoot`, which
  requires a corpus that a repo adopting the harness does not yet have.
- **FILE-006**: `src/core/harness/install/state.rs` (new) — per-asset state
  classification, in the shape `configs::FileState` already uses.
- **FILE-007**: `src/core/harness/install/plan.rs` (new) — install and
  uninstall plan construction.
- **FILE-008**: `src/core/harness/install/receipt.rs` (new) — the receipt type
  and its two locations.
- **FILE-009**: `src/core/harness/install/corpus_init.rs` (new) — `.ash/`
  scaffolding for a repo adopting the harness.
- **FILE-010**: `src/core/harness/install/check.rs` (new) — the projection
  invariants, carrying the `wire.*` and `skill.*` finding ids verbatim from the
  retired script.
- **FILE-011**: `src/core/harness/install/mod.rs` (new) — the module's public
  surface.
- **FILE-012**: `src/core/harness/mod.rs` — declare the new modules.
- **FILE-013**: `src/core/harness/root.rs` — `wired_dir`/`commands_dir` doc
  comments stop naming a script that no longer exists.
- **FILE-014**: `src/core/harness/skills.rs` — `wired` reads the shared state
  function instead of doing its own `.exists()`.
- **FILE-015**: `src/core/plan.rs` — `Action::Remove`, with `describe` and
  `privilege`.
- **FILE-016**: `src/core/exec/mod.rs` — the `Runner` arm for `Remove`.
- **FILE-017**: `src/cli/mod.rs` — `HarnessAction::{Install, Uninstall, Status}`
  and their argument structs; a new `SchemaKind` for the receipt.
- **FILE-018**: `src/cli/commands/harness.rs` — dispatch and human rendering.
- **FILE-019**: `src/cli/commands/schema.rs` — emit the receipt schema.
- **FILE-020**: `src/prompt.rs` (new) — the interactive scope/asset selector.
- **FILE-021**: `src/app.rs` — `Tab::Harness`, its state, and its keys.
- **FILE-022**: `src/ui/harness.rs` (new) — the tab.
- **FILE-023**: `src/ui/mod.rs`, `src/ui/statusbar.rs` — route the tab, add its
  hints, and the confirmation modal.
- **FILE-024**: `src/event.rs` — the event carrying harness state back from the
  blocking load.
- **FILE-025**: `justfile` — `wire` and `harness` call the binary.
- **FILE-026**: `scripts/agents-wire.sh` — deleted.
- **FILE-027**: `scripts/distil-guard.sh` — the `WIRE` invocation becomes the
  binary.
- **FILE-028**: `.github/workflows/ci.yml` — same.
- **FILE-029**: `.github/workflows/distil.yml` — the `--allowedTools` entry for
  the script is replaced.
- **FILE-030**: `.agents/README.md` — the projection section, "Adding a skill",
  and the checks table.
- **FILE-031**: `README.md` — the command table and the `.agents/` row.

## Open Questions
- Should `pinst doctor` report harness drift alongside config drift? It would
  be three lines given Phase 1's state function, but `doctor` loads a manifest
  and answers about *the machine*, while the harness answers about *the repo you
  are standing in* — the same split that kept `harness` out of `doctor` in
  `260920-wtburh`. Deferred, deliberately, and worth revisiting once the global
  scope has been lived with.
- ASSUMPTION-002 is verified inside Phase 2 rather than before it. If
  `~/.claude/commands/` turns out not to be read, the global scope installs
  skills only and the phase says so; nothing else in the plan depends on it.
