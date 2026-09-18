# pinst as a self-contained, AI-ready toolchain CLI

## Status
Done

## Context
`pinst` today is a read-only ratatui dashboard: it probes ~22 tools listed in
`registry.toml`, checks GNU Stow symlink health against `~/dotfiles`, and
reports available upgrades — but every mutation still belongs to
`~/dotfiles/scripts/bootstrap.sh` and the `justfile` (see the previous plan,
[.ash/plans/dotfiles-manager-tui/](../dotfiles-manager-tui/README.md), all
phases Done). That split leaves two sources of truth for the same toolchain,
and it leaves a fresh machine unable to do anything until someone clones the
dotfiles repo and installs `just` and `stow` first. The user now wants one
artifact: a single self-contained binary that installs, updates, and doctors
the whole stack, carries the configs with it, is driven by a manifest, and is
operable by AI agents for both bootstrapping new machines and keeping an
existing machine in sync (REQ-001 … REQ-008).

## Decision
`pinst` becomes the whole system. A **manifest** (`manifest.toml`, superseding
`registry.toml`) is the single source of truth for tools — detection, install
method, upgrade strategy, dependency edges, and tags/profiles — and also
declares the config entries. The **config tree is absorbed into this repo
under `configs/` and embedded into the binary** at compile time (the whole
dotfiles tree is 27 files / 128K), so a single downloaded binary can lay down
a machine's configs with no git clone, no network, and no GNU Stow.
Everything mutating is built as a `Plan` of ordered `Step`s and then either
printed (`--dry-run`) or executed (PAT-002), so the dry-run and the real run
can never diverge. Every command speaks a uniform **AI contract**: a versioned
JSON envelope on stdout, human logs on stderr, stable exit codes, no prompting
when not a TTY (PAT-003, GUD-002). The core is a library (`core::`) with the
CLI and the existing TUI as thin frontends over it (PAT-001), so `pinst tui`
keeps working while `pinst install|update|doctor|apply|bootstrap` become the
primary surface.

**This reverses the previous plan's central decision.** That ADR's Decision
said pinst "deliberately does **not** reimplement install/upgrade execution:
`bootstrap.sh` and `justfile` already own that idempotent logic", and its
alternatives table rejected "pinst reimplements install/upgrade actions
itself" on maintenance-surface grounds. That reasoning held while
`bootstrap.sh` was the only installer; it does not survive the new
requirements. Duplicating the tool list across a shell script and a manifest
*is* the drift this tool exists to prevent, and the "delegate to bootstrap.sh"
design cannot bootstrap a machine that does not yet have `bootstrap.sh` on it.
The old plan stays on disk as the record of how the dashboard was built; this
plan supersedes its Decision and its ALT-004 row.

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| Keep delegating installs to `bootstrap.sh` (the previous plan's decision) | Two sources of truth for one toolchain is exactly the drift problem being solved; and a fresh machine has no `bootstrap.sh` until something clones the dotfiles repo, so it cannot be the bootstrap path |
| Shell out to GNU Stow for config linking | `stow` is not present on a fresh machine (chicken-and-egg with the thing that installs it), and owning the link logic is what makes conflict backup, drift detection, and adopt possible at all |
| Fetch configs from the git repo at runtime instead of embedding | Not self-contained: needs git, network, and credentials before anything works. Embedding 128K into an already-12MB binary costs nothing and makes the config half of bootstrap work offline (CON-002) |
| Embedded configs only, no link mode | Every config edit would need a rebuild to round-trip. The dual mode (link to the source tree when it is present, materialize embedded bytes when it is not) keeps the day-to-day edit loop intact |
| A full template engine (tera/handlebars) for configs | The real need is a git identity and a couple of host paths; `${VAR}` substitution covers it with a fraction of the dependency and failure surface |
| Keep the TUI as the default entry point | An agent-facing tool must be non-interactive by default; a TUI that launches when stdout is not a TTY is a hang waiting to happen. The TUI becomes `pinst tui` |
| Hand-write the manifest JSON Schema | It would drift from the Rust types within a release. Deriving it from the same serde structs keeps `pinst schema manifest` honest by construction |

## Consequences
- **DEP-001** clap (derive) for the CLI surface; **DEP-002** schemars to derive
  the manifest JSON Schema from the same serde types; **DEP-003** include_dir
  to embed `configs/`; **DEP-004** the existing dependency set carries over
  (ratatui, crossterm, tokio, serde/toml, reqwest, regex, which, color-eyre,
  serde_json, tokio-stream). **DEP-005** was to be flate2 + tar for GitHub
  release installs; implementation dropped it — pinst downloads the asset
  natively via the reqwest it already carries and extracts with the system
  `tar`, which also keeps elevation honest for a destination like `/opt`.
- **RISK-001**: running privileged and opaque installers from a binary
  (`sudo apt`, `curl | sh`, `chsh`, writes into `/opt`) is the largest blast
  radius in this design — mitigated by the plan→apply split, `--dry-run`
  showing the exact commands, confirmation gates on privileged steps
  (SEC-002), and per-step isolation so one failure never cascades.
- **RISK-002**: embedded configs mean a rebuild to change a config —
  mitigated by source-tree link mode plus `pinst config adopt`.
- **RISK-003**: replacing Stow means owning conflict semantics — mitigated by
  preserving `bootstrap.sh`'s proven timestamped-backup behavior (GUD-001) and
  never clobbering an existing file.
- **RISK-004**: secret leakage into the binary — `~/.zshrc.secrets` is sourced
  by `.zshrc` but must never be embedded (SEC-001); the embedding path carries
  an explicit exclusion list and a test that fails on secret-looking files.
- **RISK-005**: cutover risk — `$HOME` symlinks currently point into
  `~/dotfiles`, so retiring that repo while they do would break the shell —
  mitigated by a documented cutover that re-points links first and keeps the
  old repo archived read-only until `pinst doctor` is green.
- **RISK-006**: `curl | sh` installers are unversioned and unpinnable; doctor
  reports them as "unpinnable" rather than pretending it can verify them.
- **RISK-007**: six phases is a lot of surface — every phase must leave the
  binary building, the tests passing, and the TUI working (CON-003).
- **ASSUMPTION-001**: Ubuntu/WSL with apt and sudo, carried from the previous
  plan; other platforms parse but fail with a clear error (CON-001).
- **ASSUMPTION-002**: the user's current dotfiles content is authoritative and
  moves into this repo wholesale.
- **ASSUMPTION-003**: generated-but-tracked files such as
  `nvim/.config/nvim/lazy-lock.json` are treated as ordinary configs, matching
  how the dotfiles repo tracks them today.
- **ASSUMPTION-004**: the old `~/dotfiles` git repo is archived, not deleted,
  at cutover.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Manifest schema + core domain model | [phase-01.md](phase-01.md) | Done |
| 2 | CLI shell + AI-readiness contract | [phase-02.md](phase-02.md) | Done |
| 3 | Install/update execution engine | [phase-03.md](phase-03.md) | Done |
| 4 | Embedded configs + native linking | [phase-04.md](phase-04.md) | Done |
| 5 | Doctor + apply/sync convergence | [phase-05.md](phase-05.md) | Done |
| 6 | Distribution, cutover, agent docs | [phase-06.md](phase-06.md) | Done |

## Affected Files
- FILE-001: `manifest.toml` — new single source of truth, supersedes `registry.toml` (deleted)
- FILE-002: `src/core/manifest.rs` — manifest types, loader, validation, JSON Schema derivation
- FILE-003: `src/core/graph.rs` — dependency topological ordering, cycle detection, profile/tag selection
- FILE-004: `src/core/mod.rs` — core module root and re-exports
- FILE-005: `src/registry/mod.rs` — deleted, superseded by FILE-002
- FILE-006: `src/cli/output.rs` — JSON envelope, status, exit codes, stdout/stderr split
- FILE-007: `src/cli/mod.rs` — clap `Cli`, global flags, `Commands` enum, TTY detection
- FILE-008: `src/main.rs` — command dispatch and exit-code mapping
- FILE-009: `src/cli/commands/list.rs` — tool inventory with probe status
- FILE-010: `src/cli/commands/schema.rs` — emits the manifest JSON Schema
- FILE-011: `src/core/plan.rs` — `Plan`/`Step`/`StepKind`/`StepOutcome`
- FILE-012: `src/core/exec/mod.rs` — `Executor` trait, command runner, privilege gating
- FILE-013: `src/core/exec/apt.rs` — apt executor
- FILE-014: `src/core/exec/cargo.rs` — cargo executor
- FILE-015: `src/core/exec/curl_script.rs` — `curl | sh` installer executor
- FILE-016: `src/core/exec/github_release.rs` — release tarball/binary executor
- FILE-017: `src/core/exec/nvm.rs` — nvm-managed node executor
- FILE-018: `src/core/exec/git_clone.rs` — git-clone executor (omz plugins)
- FILE-019: `src/core/engine.rs` — plan construction and isolated execution with summary
- FILE-020: `src/cli/commands/install.rs` — `pinst install`
- FILE-021: `src/cli/commands/update.rs` — `pinst update`
- FILE-022: `src/cli/commands/plan.rs` — `pinst plan`
- FILE-023: `configs/**` — the absorbed dotfiles tree (nvim, zsh, herdr, git)
- FILE-024: `src/core/template.rs` — `${VAR}` substitution and values file
- FILE-025: `src/core/configs.rs` — embedding, source-tree resolution, link/materialize, backup, drift, adopt
- FILE-026: `src/cli/commands/config.rs` — `pinst config status|apply|adopt|diff`
- FILE-027: `src/core/doctor.rs` — `Finding` type and the composed checks
- FILE-028: `src/cli/commands/doctor.rs` — `pinst doctor [--fix]`
- FILE-029: `src/cli/commands/apply.rs` — `pinst apply` full convergence
- FILE-030: `src/ui/health.rs` + `src/app.rs` — TUI health view rewired onto doctor findings
- FILE-031: `Cargo.toml` — new dependencies and the release profile
- FILE-032: `src/cli/commands/bootstrap.rs` — `pinst bootstrap` one-shot
- FILE-033: `scripts/install.sh` — fresh-machine install one-liner
- FILE-034: `AGENTS.md` — the agent-facing contract and extension guide
- FILE-035: `README.md` — human docs and the `~/dotfiles` cutover procedure

## Open Questions
None. ASSUMPTION-001 … ASSUMPTION-004 are accepted as stated; the platform
question (CON-001) is settled by failing loudly on unsupported platforms
rather than guessing.
