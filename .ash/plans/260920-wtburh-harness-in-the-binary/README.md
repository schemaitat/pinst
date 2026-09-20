---
id: 260920-wtburh
slug: harness-in-the-binary
status: In Progress
created: 2026-09-20
updated: 2026-09-20
areas: [cli, harness, agents]
summary: Port the agent harness from scripts/ash.sh into a first-class `pinst harness` subcommand family, so the binary that ships carries the corpus checks it currently leaves behind in bash.
files_touched: [src/core/harness/mod.rs, src/core/harness/root.rs, src/core/harness/frontmatter.rs, src/core/harness/id.rs, src/core/harness/corpus.rs, src/core/harness/index.rs, src/core/harness/check.rs, src/core/harness/skills.rs, src/core/harness/evidence.rs, src/core/harness/transcripts.rs, src/core/mod.rs, src/cli/mod.rs, src/cli/commands/harness.rs, src/cli/commands/mod.rs, src/cli/commands/schema.rs, justfile, scripts/ash.sh, .agents/README.md, .agents/skills/plan-write/SKILL.md, .agents/skills/plan-learnings/SKILL.md, .claude/commands/plan.md, .claude/commands/implement.md, .ash/INDEX.md, README.md]
---

# The harness moves into the binary

## Context
The harness that keeps this repo's agent corpus honest is 1054 lines of bash
in `scripts/ash.sh`, plus 200 more in `scripts/agents-wire.sh`. It validates
every plan under `.ash/`, generates `.ash/INDEX.md`, grades each skill in
`.agents/skills/` against its own declared contract, and mints plan ids. It is
wired into `just harness` and therefore into `just qc`, and it already speaks
pinst's contract by hand: a JSON envelope with `schema_version`, findings with
a stable `id` and a `remediation`, and exit codes `0` clean / `2` usage / `3`
found things to act on (CON-001).

Speaking that contract by hand is the problem. `report()`, `finding()`, the
envelope printer and the severity tally in `ash.sh` are a second, `printf`-based
implementation of what `src/cli/output.rs` and `src/core/doctor.rs` already
define in types (PAT-002) — and the bash one is the copy nothing tests. More
concretely, the harness is the one part of this repo's tooling that a released
pinst cannot do: the binary ships self-contained, installs from a GitHub
release with no clone, and carries `configs/` and now `docs/tools/` compiled
in — but `pinst harness check` does not exist, so a machine with the binary and
a checkout of some other repo that has adopted `.ash/` has no way to run the
checks (REQ-004, REQ-005).

The port is also the cheapest moment to fix a known weakness. `ash.sh` reads
YAML frontmatter with `awk` one line at a time, and LESSON-008 records what
that costs: six `SKILL.md` files "had never been valid YAML because only
hand-written parsers had ever read them". A line-oriented reader that silently
skips what it does not recognise cannot report a malformed document — it
reports a *missing key* on a document that is merely shaped unexpectedly, or
nothing at all (RISK-003).

Finally, this reverses a decision recorded one day ago. Plan `260919-vldfei`
rejected exactly this option outright, on two grounds: pinst's domain is the
machine's toolchain rather than this repo's prose corpus, and the checks are
"deliberately shell so they run with no build step on a machine that has not
compiled anything". The second ground has since expired — `260919-zeuuaj`
made pinst a downloaded binary and `scripts/install.sh` puts it on `PATH`
without a toolchain, so "no build step" now argues *for* the binary and against
the script. The first ground is a real cost this plan accepts rather than
refutes, and it is written up under Consequences (RISK-001).

## Decision
Add `pinst harness <index|check|skills|new-id>`, implemented in Rust under
`src/core/harness/` with a thin command module at
`src/cli/commands/harness.rs`, exactly as `docs` is laid out (PAT-001,
PAT-003). Every finding keeps the **same id string** it has today, so an agent
matching on `phase.logged-not-done`, and the `lesson.unenforced` check that
greps for a lesson's `**Check:**` id, both keep working across the cutover
(REQ-002). Findings become `core::doctor::Finding` values carried in the
existing `Envelope`, which deletes the hand-rolled JSON printer rather than
translating it.

**Nothing is embedded, and that is the correct answer.** `configs/` and
`docs/tools/` are compiled in because they are content *pinst ships to a
machine*. `.ash/` is content *the checked repo owns* — embedding this repo's
plan corpus into the binary would ship one project's history to every
machine and answer questions about the wrong corpus. What makes the command
self-contained is that the **logic** is in the binary; the corpus is
discovered on disk. So `include_dir` plays no part here, and the discovery
path is deliberately *not* `core::source::resolve`, which only accepts a
directory sitting next to a `manifest.toml` and would therefore find pinst's
own checkout instead of the repo the user is standing in (ALT-005, ALT-007).
`harness` also becomes the first command family besides `schema` that does not
load a manifest at all (CON-002).

Retire `scripts/ash.sh` in the same plan rather than keeping it beside the
port. Two implementations of one set of invariants is precisely the drift this
harness exists to detect, and it would be detecting it about itself
(ASSUMPTION-002). `scripts/agents-wire.sh` stays: skill projection is a
different job with its own nine finding ids, and folding it in would double the
size of this change for no part of the stated goal (RISK-006).

Frontmatter gets a **strict** parser rather than a line-oriented one: it
accepts the flat scalar-and-inline-list subset every document in the corpus
actually uses, and anything outside that subset becomes a
`plan.frontmatter-unparsed` finding naming the line. That is the safe
direction — LESSON-008's failure was a parser that skipped what it did not
understand, and a parser that *refuses* what it does not understand cannot
reproduce it (ALT-006, RISK-003).

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Leave the harness in bash (the status quo, and `260919-vldfei`'s ALT-004) | Its stated justification was "no build step on a machine that has not compiled anything". That machine now installs a pinst binary from a GitHub release and gets `configs/` and `docs/` with it; bash is the thing it would additionally need to have cloned. The contract-by-`printf`, the untested JSON printer and the awk frontmatter reader are all costs the Rust side already pays for once |
| ALT-002: Keep `ash.sh` and have `pinst harness` shell out to it | Ships the binary's headline property — self-contained — as a lie: the command would fail on exactly the fresh machine it exists for. It also keeps the bash as the real implementation while adding a second surface to maintain |
| ALT-003: Port to Rust but keep `ash.sh` alongside as a no-build fallback | Two implementations of the same 26 invariants, drifting silently, with `just qc` exercising one of them. The harness's own LESSON-008 is that an invariant stated twice is an invariant already drifting |
| ALT-004: A separate `ash` binary in a cargo workspace | Doubles the release surface — a second artifact, a second install path, a second entry in `manifest.toml` — to avoid one subcommand on a binary that already has ten. The shared `Envelope`, `Finding` and exit-code types would have to move to a third crate to be reused |
| ALT-005: Embed `.ash/` into the binary with `include_dir`, as `configs/` and `docs/tools/` are | Those trees are content pinst *ships*; the corpus is content the checked repo *owns*. An embedded copy would answer about this repo's plans while standing in another repo, and go stale the moment a plan is written. The self-contained property comes from the logic being compiled in, not the data |
| ALT-006: A maintained YAML crate for frontmatter | Correct in principle and still the fallback, but it adds a dependency to a binary explicitly tuned for size (`opt-level = "z"`, `lto`, `strip`), and the YAML ecosystem's maintained option could not be confirmed from this session — the sandbox has no crates.io access (ASSUMPTION-004). The strict-subset parser needs no dependency and fails loudly, which is the property LESSON-008 actually asks for |
| ALT-007: Discover the corpus with `core::source::resolve`, reusing the configs/docs pattern | `checkout_tree` requires a `manifest.toml` beside the target directory, and falls back to the binary's own ancestors. Run in a repo that has `.ash/` but is not pinst, it finds nothing; run from `~/.local/bin`, it could find pinst's own checkout. Corpus discovery walks up for `.ash/` itself |
| ALT-008: Port `index`, `check` and `new-id` but leave `skills` in bash | `skills` is the half that reads the same corpus model as `check` — plan dirs, `learnings.md` issues, `LEARNINGS.md` lessons. Splitting it across two languages means writing that model twice, which is the drift ALT-003 was rejected for |

## Consequences
- **DEP-001**: no new crate, if the strict frontmatter parser is taken.
  `serde_json`, `regex`, `clap` and `schemars` are already in the tree.
  **DEP-002**: `git`, unchanged — it is invoked by the `evidence:` one-liners,
  not by pinst. **DEP-003**: `sh`, to run `evidence:`; the contract is "a shell
  one-liner", so the shell is not removable. **DEP-004**: `python3` is
  *removed* as a dependency — the `--transcripts` JSONL scan becomes
  `serde_json`.
- **SEC-001**: `harness skills` executes a shell command taken from a file in
  the repo being checked. `ash.sh` did too, but the blast radius changes: a
  script you had to clone becomes a binary you installed and trust. Mitigated
  by keeping the 30-second timeout, adding `--no-evidence`, and documenting it
  where `evidence:` is defined — but the default stays "on", because a
  measurement that has to be asked for is a measurement nobody takes
  (ASSUMPTION-003).
- **SEC-002**: the `--transcripts` path still reads conversation text from
  outside the repo and may emit **derived counts only** — never a quoted line,
  a path, or an argument string — and writes nothing into `.ash/`. Carried
  forward verbatim from `260919-vldfei`.
- **CON-003**: `check` stays silent on a clean corpus and `skills` stays out of
  `just qc`. `report()`'s rule — any finding, any severity, exit 3 — is what
  makes that necessary, and it survives the port unchanged.
- **GUD-001**: every ported check keeps comparing the corpus against a record
  written *after* the fact (LESSON-009). Nothing in this plan adds a check;
  the set of 26 findings is exactly the set that exists today (REQ-002).
- **RISK-001**: this reverses `260919-vldfei`'s ALT-004, whose surviving
  argument is that pinst's domain is the machine's toolchain rather than one
  repo's prose corpus. The cost is real: someone who installs pinst to manage
  their tools gets a `harness` family that does nothing for them until they
  adopt `.ash/`. The counter is that pinst's domain widened once already, with
  `docs`, from "what is installed" to "how to use it", and that this repo's own
  dev tooling is a thing pinst is for. Recorded rather than argued away.
- **RISK-002**: the no-build-step property is genuinely lost. `just index` and
  `just harness` will need a compiled binary. In `qc` this is free — the recipe
  order is `fmt-check lint test harness`, so a build has already happened by
  the time `harness` runs — but a bare `just index` on a cold tree now pays for
  a build.
- **RISK-003**: a hand-written frontmatter parser is how LESSON-008 happened.
  Mitigated by making the parser strict: it emits a finding for anything
  outside the accepted subset instead of skipping it, and the accepted subset
  is pinned by a test that parses every document in the real corpus.
- **RISK-004**: `lesson.unenforced` greps `scripts/` for a lesson's
  `**Check:**` id. Three of the four mechanized lessons name checks that move
  into `src/`, so the search root must widen in the same change or the check
  starts reporting its own migration as a failure.
- **RISK-005**: finding ids are format strings in Rust
  (`plan.id-mismatch.{name}`), so the literal a lesson names —
  `phase.logged-not-done` — must remain a contiguous literal in the source for
  the grep in RISK-004 to find it. Ids assembled from fragments would break
  `lesson.unenforced` silently.
- **RISK-006**: `just harness` still calls `scripts/agents-wire.sh --check`, so
  "the harness needs no `scripts/`" is true of the corpus checks and not yet of
  the recipe. Porting wire is a follow-up, not this plan.
- **ASSUMPTION-001**: this branch rebases onto `origin/main` (`e96fac0`) before
  phase 1, so `build.rs`, `src/core/source.rs` and the `docs` layout are
  present to mirror. Verified by query: the `260920-tensvp` plan landed and the
  branch is one commit behind.
- **ASSUMPTION-002**: `scripts/ash.sh` is deleted rather than kept as a shim.
  Decided by default, cheap to reverse (restore the file and one `just` line).
- **ASSUMPTION-003**: evidence execution defaults to on, with `--no-evidence`
  to opt out.
- **ASSUMPTION-004**: the strict-subset frontmatter parser is preferred over a
  YAML crate, on a crate-health question this session could not verify by query
  because the sandbox blocks crates.io. If the parser turns out to need more
  than the accepted subset, ALT-006 is one `cargo add` away.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | The corpus model, and `pinst harness new-id` | [phase-01.md](phase-01.md) | Done |
| 2 | `pinst harness index` | [phase-02.md](phase-02.md) | Proposed |
| 3 | `pinst harness check` — every corpus invariant | [phase-03.md](phase-03.md) | Proposed |
| 4 | `pinst harness skills` — the graded report | [phase-04.md](phase-04.md) | Proposed |
| 5 | Cutover: retire the script, rewire the callers | [phase-05.md](phase-05.md) | Proposed |

## Affected Files
- **FILE-001** `src/core/harness/mod.rs` — the subsystem root: the `Corpus`
  type, the plan/phase model, and the module tree, mirroring
  `src/core/docs/mod.rs`.
- **FILE-002** `src/core/harness/root.rs` — corpus discovery: walk up from the
  working directory for a directory containing `.ash/`, honouring an explicit
  `--root` and `PINST_ASH_DIR`.
- **FILE-003** `src/core/harness/frontmatter.rs` — the strict frontmatter
  parser and its accepted subset.
- **FILE-004** `src/core/harness/id.rs` — `mint_id`: `<yymmdd>-<six lowercase
  letters>`, random half, optional ISO date.
- **FILE-005** `src/core/harness/corpus.rs` — plan directories, phase files,
  `learnings.md` issues, `LEARNINGS.md` lessons, `CHANGELOG.log` entries.
- **FILE-006** `src/core/harness/index.rs` — `render_index`, byte-compatible
  with the bash generator apart from the attribution line.
- **FILE-007** `src/core/harness/check.rs` — the 26 corpus findings, ids
  unchanged.
- **FILE-008** `src/core/harness/skills.rs` — the per-skill grade, the gap
  rows, the agenda.
- **FILE-009** `src/core/harness/evidence.rs` — running an `evidence:`
  one-liner under a timeout, with `ASH_WINDOW`/`ASH_RANGE` set.
- **FILE-010** `src/core/harness/transcripts.rs` — the opt-in invocation count,
  ported from the embedded `python3` block.
- **FILE-011** `src/core/mod.rs` — declare the `harness` module.
- **FILE-012** `src/cli/mod.rs` — `Commands::Harness`, `HarnessArgs`,
  `HarnessAction`, the `name()` arm and the `dispatch` arm.
- **FILE-013** `src/cli/commands/harness.rs` — argument handling and rendering
  for the four actions.
- **FILE-014** `src/cli/commands/mod.rs` — `pub mod harness;`.
- **FILE-015** `src/cli/commands/schema.rs` — a `SchemaKind::Harness` emitting
  the schema for the corpus/skill report item types.
- **FILE-016** `justfile` — `index`, `harness` and `review` call the binary;
  `harness` keeps its `agents-wire.sh --check` half.
- **FILE-017** `scripts/ash.sh` — deleted.
- **FILE-018** `.agents/README.md` — every `scripts/ash.sh …` invocation
  becomes `pinst harness …`; the "The checks" and "Grading the skills"
  sections gain the new command names.
- **FILE-019** `.agents/skills/plan-write/SKILL.md` — Step 2's `new-id` call
  and Step 7's `index`/`check` references.
- **FILE-020** `.agents/skills/plan-learnings/SKILL.md` — the `check` and
  `skills` references, and the note that `**Check:**` ids are now searched for
  under `src/` as well as `scripts/`.
- **FILE-021** `.claude/commands/plan.md` — the pre-loaded `new-id` call.
- **FILE-022** `.claude/commands/implement.md` — the pre-loaded `check` call.
- **FILE-023** `.ash/INDEX.md` — regenerated, so its "Generated by" attribution
  names the new command.
- **FILE-024** `README.md` — the command table gains `harness`.

## Open Questions
Four decisions were made by default because the requester was not available
while this was written. Each is flagged here rather than buried, and each is
cheap to reverse:

- ASSUMPTION-002 — delete `scripts/ash.sh`, or keep it as a no-build
  fallback? Deleting is recommended (one implementation of one set of
  invariants); the cost is RISK-002.
- ASSUMPTION-003 — should `harness skills` run `evidence:` shell by
  default? Recommended yes with `--no-evidence`, but SEC-001 is a genuine
  escalation from a cloned script to an installed binary, and inverting the
  default is a one-line change.
- ASSUMPTION-004 — strict-subset frontmatter parser, or a YAML crate? The
  crate-health question could not be answered from this sandbox, which is
  itself a reason to prefer the no-dependency option first.
- RISK-006 — `scripts/agents-wire.sh` is deliberately out of scope, which
  leaves `just harness` half in bash. Folding it in (as `pinst harness wire`)
  would finish the story and add roughly a phase.
