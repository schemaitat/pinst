---
id: 260920-wtburh
slug: harness-in-the-binary
phase: 1
status: Done
---

# Phase 1 — The corpus model, and `pinst harness new-id`

## Goal
Stand up `src/core/harness/` with the three things every later phase reads
through — corpus discovery, a strict frontmatter parser, and the plan/phase
model — and ship the one subcommand that is small enough to prove the wiring:
`pinst harness new-id`.

## Why this phase exists
Discovery, frontmatter and the corpus model are shared by `index`, `check` and
`skills` alike; written inside whichever of those landed first they would be
shaped by that one caller and refactored twice. Pairing them with `new-id` is
LESSON-016 applied literally — do not land an API before the code that calls
it. `new-id` is the only action whose implementation is smaller than the
plumbing it needs, so it is the cheapest honest consumer, and it is also the
one the `/plan` command invokes on every single planning session, which makes
the cutover for it observable immediately.

## Steps
- [x] TASK-001: rebase this branch onto `origin/main` (`e96fac0`) — Why: the
      `docs` subsystem, `build.rs` and `src/core/source.rs` landed there, and
      this plan mirrors their layout (ASSUMPTION-001). Nothing below is
      meaningful against `0cc43ca`.
- [x] TASK-002: `src/core/harness/root.rs` (FILE-002) — a `CorpusRoot` that
      resolves in three steps: an explicit path, then `$PINST_ASH_DIR`, then a
      walk up from the working directory for the first ancestor containing a
      `.ash` directory. Why: deliberately *not* `core::source::resolve`
      (ALT-007) — that requires a `manifest.toml` neighbour and falls back to
      the binary's own ancestors, so it would find pinst's checkout while the
      user stands in another repo.
- [x] TASK-003: `src/core/harness/frontmatter.rs` (FILE-003) — parse the block
      between the first two `---` lines into an ordered map. Accept `key:
      scalar`, `key: 'single-quoted with '' escapes'`, `key: "double
      quoted"`, and `key: [a, b, c]`. Return a typed error naming the line
      number for anything else — a nested mapping, a block scalar, a
      multi-line list. Why: RISK-003 — the LESSON-008 failure was a parser
      that skipped what it did not understand, so this one refuses.
- [x] TASK-004: `src/core/harness/frontmatter.rs` — a test that parses every
      `.ash/plans/*/README.md`, every `phase-*.md` and every
      `.agents/skills/*/SKILL.md` in the real tree and asserts no error. Why:
      this is what pins the "accepted subset" to reality rather than to an
      author's guess, and it is the test that would have caught LESSON-008's
      six invalid files.
- [x] TASK-005: `src/core/harness/id.rs` (FILE-004) — `mint_id(date:
      Option<&str>) -> String` producing `<yymmdd>-<six lowercase letters>`,
      the letter half from the OS RNG, the date half defaulting to today UTC
      and otherwise rendered from an ISO `YYYY-MM-DD`. Why: `yymmdd` so string
      order is date order, random so parallel worktrees cannot collide — both
      rationales already written down in `.agents/README.md` and preserved as
      doc comments here.
- [x] TASK-006: `src/core/harness/corpus.rs` (FILE-005) and
      `src/core/harness/mod.rs` (FILE-001) — a `Corpus` that enumerates plan
      directories under `<root>/.ash/plans`, splits each name into `id` and
      `slug`, loads its `README.md` frontmatter, its `phase-NN.md` files, and
      the presence of `learnings.md`. No findings yet; this phase only models.
- [x] TASK-007: `src/core/mod.rs` (FILE-011), `src/cli/mod.rs` (FILE-012),
      `src/cli/commands/mod.rs` (FILE-014) and `src/cli/commands/harness.rs`
      (FILE-013) — declare the module, add `Commands::Harness(HarnessArgs)`
      with a `HarnessAction` enum, its `name()` arm returning `"harness"`, and
      its `dispatch` arm. Implement `new-id` only; the other three actions
      are added by the phases that own them. Why: `harness` must not call
      `super::load_manifest` (CON-002) — it is the first family besides
      `schema` with no manifest.
- [x] TASK-008: `src/cli/commands/harness.rs` — `new-id` prints the bare id on
      stdout in human mode and an `Envelope` with one item in `--json` mode.
      Why: `scripts/ash.sh new-id` prints a bare line today and `/plan`
      substitutes it directly into a prompt; a JSON wrapper on the default
      path would break that caller at the cutover.

## Trade-offs & risks
RISK-003 is the live one: TASK-003 writes a parser by hand, which is how
LESSON-008 happened. TASK-004 is the mitigation and has to land with it, not
after. ASSUMPTION-004 records that ALT-006 — a YAML crate — was preferred *in
principle* and rejected on a dependency-health question this session could not
verify by query; if TASK-004 shows the corpus already uses constructs outside
the subset, take the crate instead of widening the hand-written parser.

CON-004 applies from here on: `just qc` runs `clippy -D warnings`, so nothing
in `corpus.rs` may be written ahead of a caller. Where a field is genuinely
needed by phase 3 and unused in phase 1, either omit it or mark it with an
`#[cfg_attr(not(test), allow(dead_code))]` whose comment names phase 3
(LESSON-016).

## Done criteria
- **TEST-001**: `cargo run -- harness new-id` prints a string matching
  `^[0-9]{6}-[a-z]{6}$`, and 100 successive calls produce 100 distinct ids.
- **TEST-002**: `cargo run -- harness new-id --json` emits a valid envelope
  with `command: "harness new-id"` and exit code 0.
- **TEST-003**: `cargo run -- harness new-id 2026-09-19` prints an id starting
  `260919-`, matching `scripts/ash.sh new-id 2026-09-19`.
- **TEST-004**: the TASK-004 test passes over the real tree, and a unit test
  asserts that a frontmatter block containing a nested mapping produces an
  error naming its line number.
- **TEST-005**: `cargo run -- harness new-id` succeeds from a directory with
  no `.ash/` anywhere above it — minting an id must not require a corpus.
- **TEST-006**: `just qc` is green.
