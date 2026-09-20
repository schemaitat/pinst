---
id: 260920-tensvp
slug: tool-docs-explorer
status: In Progress
created: 2026-09-20
updated: 2026-09-20
areas: [cli, docs, manifest, tui]
summary: Give pinst a baked-in, searchable catalogue of how to use the tools it installs, so an agent can find the right tool and its exact invocation in one call.
files_touched: [src/core/source.rs, src/core/docs/mod.rs, src/core/docs/page.rs, src/core/docs/capture.rs, src/core/docs/search.rs, src/core/configs.rs, src/core/manifest.rs, src/core/mod.rs, src/cli/mod.rs, src/cli/commands/docs.rs, src/cli/commands/mod.rs, src/cli/commands/schema.rs, docs/tools/, manifest.toml, README.md, .agents/skills/pinst/SKILL.md, src/app.rs, src/event.rs, src/ui/mod.rs, src/ui/docs.rs]
---

# A baked-in, searchable catalogue of tool usage

## Context
pinst already knows every tool on this machine — the manifest declares 26 of
them with a name, a summary, tags, dependencies and an install method, and
probing says which are present and at what version. What it does not know is
how to *use* any of them. An agent that needs to search a tree today has to
guess that `rg` exists, guess its flags, run it, read the error, and try
again; the knowledge is either in the model's weights (stale, generic) or on
the machine behind a `--help` call it must first think to make (REQ-001).

The gap is exactly one step past what pinst already owns. The manifest is the
list of what is here; nothing says what each entry is *for*, which invocation
is the right one on this machine, or which tool to reach for given an
intention like "search a tree" or "run a task" (REQ-002). And `--help` is the
wrong answer on its own: it is written for a human who already knows they
want this tool, it lists every flag with equal weight, and it cannot say
"prefer `fd` over `find` here" (REQ-003).

pinst is also already the right carrier. It ships as one self-contained binary
with the config tree compiled in, so a freshly provisioned machine has the
content with no clone and no network (REQ-004); its output contract — a
versioned JSON envelope on stdout, human commentary on stderr, exit codes that
carry the verdict — is precisely what an agent needs to consume a lookup
without parsing prose (CON-002).

## Decision
Add a `pinst docs` command family over a new catalogue of **authored pages**,
one TOML file per tool at `docs/tools/<name>.toml`, embedded into the binary
with `include_dir` exactly as `configs/` is (PAT-002). A page is short and
agent-shaped: what the tool is, when to reach for it, a handful of named
recipes as literal command lines, gotchas, and see-also links to sibling
tools. It is prose a human wrote on purpose, not a dump of `--help`.

Three verbs make it usable in one call each (PAT-001, mirroring
`pinst config <action>`): `docs search <query>` ranks the catalogue by
intent and returns the matching recipes inline, so a single search is
usually enough to act; `docs show <tool>` returns one page in full; and
`docs dump` emits the whole catalogue as one document for an agent that
would rather load it into context than call anything again (REQ-001,
REQ-002, REQ-003).

Where no page exists yet, `docs show` falls back to **capturing** the tool's
own help on the machine — `<bin> --help` by default, or a `help_cmd` the
manifest overrides — cached under `$XDG_CACHE_HOME/pinst` keyed by the
detected version, so it re-captures exactly when the tool changes (REQ-005).
`docs adopt <tool>` writes that capture back into `docs/tools/<name>.toml`
as a `status = "draft"` page, which is the same round-trip `pinst config
adopt` already offers for configs: the machine seeds the repo, a human
promotes the draft to `authored`.

Coverage is **reported, never enforced**. `docs status` says which tools have
a page, which are drafts, and which have only a capture (REQ-006) — but a missing page
is not a doctor finding and does not fail `just qc`, because on the day this
ships almost every tool is missing one and a gate that is red by design is a
gate people learn to skip (GUD-001, LESSON-011). The invariant that *is*
mechanized is the one that can only be true or broken: a page whose name
matches no manifest tool, or that fails to parse, fails `cargo test` and
therefore `just qc` (LESSON-008).

Scope stays inside what the manifest declares (CON-001). The catalogue and
the toolchain then have one source of truth and one act of maintenance —
adding a tool to `manifest.toml` is what creates a slot for its page — and
every search result carries install status and version for free.

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: Vendor the tldr-pages corpus into the binary | Thousands of pages for tools this machine does not have, an upstream sync job, an attribution obligation, and generic content that cannot say which invocation is right *here*. The 26 tools that matter are worth 26 authored pages |
| ALT-002: Capture `--help`/`man` only, author nothing | Gets the shape of the content wrong. `--help` answers "what are this tool's flags", and the question an agent actually has is "which tool, and what do I type" — a ranking and a recommendation, which no tool's own help contains. It also requires the tool installed, so a fresh machine answers nothing |
| ALT-003: Put the prose in `manifest.toml` | The manifest is the machine contract for provisioning and is parsed on every install path; adding paragraphs of documentation to it bloats a stable interface and couples doc edits to schema-versioned data |
| ALT-004: Markdown pages with YAML frontmatter | Needs a frontmatter parser and a second file format in a repo that already parses TOML for its one source of truth. TOML pages parse with the existing `toml` dep and serialize to the `--json` envelope for free; multi-line `"""` strings carry the prose fine |
| ALT-005: Index every binary on `PATH`, not just manifest tools | Makes the answer machine-dependent — a fresh machine and a provisioned one would return different catalogues for the same query — and gives up the install status, tags and dependency edges that make a manifest-backed result actionable |
| ALT-006: Ship it as an MCP server instead of a CLI command | A second protocol surface and a long-lived process to supervise, for a lookup that is already a single non-interactive command with JSON on stdout. Any harness that can run `pinst` can consume this; nothing new has to be spoken |
| ALT-007: Add a fuzzy-match crate for search | A 26-entry catalogue over a few hundred keywords is searched exactly by scored substring matching in well under a millisecond. A new dependency in a binary tuned for size (`opt-level = "z"`, LTO, stripped) has to earn its way in, and this one buys ranking nobody can explain |
| ALT-008: Make a tool without a page a `doctor` finding | Fires on the normal state from the first commit until the catalogue is complete, so `just qc` is red by design for the whole project — LESSON-011 exactly. Coverage is a report (`docs status`); the orphan/parse invariant is the part that becomes a test |
| ALT-009: Name the command `explore` (as the branch does) | `docs <action>` mirrors the one other action-bearing command, `config <action>`, and names what the command returns rather than what a human does with it. Recorded as ASSUMPTION-001 — it is a one-line rename if the requester prefers the branch's word |
| ALT-010: Commit the captured `--help` text as the catalogue | Freezes one machine's tool versions into the repo, re-churns on every upgrade, and produces pages that are long, unranked and untrue on the next machine. The capture belongs in the version-keyed cache; only a human-promoted draft belongs in git |

## Consequences
- **DEP-001** nothing new. `toml`, `include_dir`, `schemars`, `serde` and
  `tokio` are already dependencies; the catalogue is parsed, embedded,
  schema-generated and captured with what the binary already carries
  (CON-003).
- **DEP-002** `docs/tools/` becomes a build input: `include_dir!` means
  editing a page rebuilds the binary. On a machine with the checkout present
  the tree is read live instead, so authoring does not require a rebuild to
  test — the same tree-or-embedded resolution `configs/` already uses
  (PAT-002).
- **SEC-001**: capture executes an installed binary. It runs only for tools
  the manifest declares, only the manifest's `help_cmd` or
  `<detect.bin> --help`, never for a name that came from a search query, and
  never during `dump`. It runs with a timeout, a hard output cap, and
  `NO_COLOR=1`/`TERM=dumb` in the child environment. A manifest string reaching
  `sh -c` is existing, accepted behaviour — `detect.command` and
  `post_install.command` already do it — so the trust boundary does not move.
- **CON-002** holds unchanged: every `docs` action speaks the same envelope
  with `schema_version` 1, puts human output on stderr, and never prompts in
  `--json` mode. `adopt` is the only mutating action and therefore the only
  one that honours `--dry-run`/`--yes`.
- **CON-004**: `pinst schema docs` joins `schema manifest|output`, derived
  from the same Rust type the loader uses, so an agent can author a page
  without reading source.
- **GUD-001** is the shape of every check this plan adds: report what is
  merely incomplete, fail only on what is broken.
- **RISK-001**: a page drifts from the version installed. Pages carry an
  optional `verified_with`, and `docs status` surfaces a page verified
  against an older version than the one probed — as a line in a report, not a
  finding.
- **RISK-002**: catalogue growth inflates the binary a fresh machine
  downloads. Pages are prose-only, no images, and phase 5 records the
  measured `just dist` delta; if short pages for 26 tools cost more than a
  few tens of KB, that is a fact worth having before the catalogue grows.
- **RISK-003**: a wrong page is worse than no page, because an agent will act
  on it without checking. Mitigated by `status = "draft" | "authored"`
  travelling with every page through search, show and dump, and by phase 5's
  done criterion that every recipe in an `authored` page was actually run.
- **RISK-004**: scope creep into a general documentation system — man page
  rendering, tldr sync, per-flag indexes. The boundary is CON-001: manifest
  tools, one page each, recipes not reference manuals.
- **ASSUMPTION-001**: the command is `docs`, not the branch's `explore`
  (ALT-009). Cheap to reverse.
- **ASSUMPTION-002**: TOML pages (ALT-004). Reversible, but only before the
  catalogue is authored in phase 5 — that is the point of no return, and it
  is why the format ships and gets exercised by four phases of commands
  before any bulk content is written.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | The page format and the embedded catalogue | [phase-01.md](phase-01.md) | Done |
| 2 | Read the catalogue: `docs show` and `docs status` | [phase-02.md](phase-02.md) | Done |
| 3 | Capture what is not written yet, and adopt it | [phase-03.md](phase-03.md) | Proposed |
| 4 | The one-call surface: `docs search` and `docs dump` | [phase-04.md](phase-04.md) | Proposed |
| 5 | Author the catalogue | [phase-05.md](phase-05.md) | Proposed |
| 6 | Publish the contract and make it browsable | [phase-06.md](phase-06.md) | Proposed |

## Affected Files
- **FILE-001** `src/core/source.rs` (new) — `Source` plus the tree-resolution
  helpers lifted out of `configs.rs`, parameterized by subdirectory and
  override env var, so configs and docs resolve tree-or-embedded the same way.
- **FILE-002** `src/core/configs.rs` — keeps its public surface; the
  resolution internals delegate to FILE-001.
- **FILE-003** `src/core/docs/` (new) — `page.rs` (the `ToolDoc` type, parse,
  `JsonSchema`), `mod.rs` (catalogue load and lookup), `capture.rs`
  (help capture, cache), `search.rs` (ranking).
- **FILE-004** `src/core/mod.rs` — registers the `docs` and `source` modules.
- **FILE-005** `src/core/manifest.rs` — optional `help_cmd` on `Tool`.
- **FILE-006** `src/cli/mod.rs` — `Commands::Docs`, `DocsArgs`/`DocsAction`,
  dispatch, `name()`, and `SchemaKind::Docs`.
- **FILE-007** `src/cli/commands/docs.rs` (new) — the five actions.
- **FILE-008** `src/cli/commands/mod.rs` — module registration.
- **FILE-009** `src/cli/commands/schema.rs` — emits the page schema.
- **FILE-010** `docs/tools/*.toml` (new tree) — the catalogue itself.
- **FILE-011** `manifest.toml` — `help_cmd` for the tools whose help is not
  `<bin> --help`.
- **FILE-012** `.agents/skills/pinst/SKILL.md` — the `docs` contract: actions,
  items, exit codes, and when an agent should reach for it.
- **FILE-013** `README.md` — commands table and a section on the catalogue.
- **FILE-014** `src/app.rs`, `src/event.rs`, `src/ui/mod.rs`,
  `src/ui/docs.rs` (new) — the TUI Docs tab.

## Open Questions
- **ASSUMPTION-001** (`docs` vs `explore`) and **ASSUMPTION-002** (TOML vs
  Markdown pages) are both decided here by default and both named in the
  alternatives table. Everything else the plan assumes about the codebase —
  the embedding mechanism, the envelope, the exit codes, the existing
  `adopt` round-trip — was read out of the source before this was written.
