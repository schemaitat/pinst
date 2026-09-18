# pinst: a ratatui TUI dashboard for ~/dotfiles

## Status
Done

## Context
`~/dotfiles` is a GNU Stow–managed repo (packages: `nvim`, `zsh`, `herdr`, `git`,
symlinked into `$HOME` via `just install`) plus `scripts/bootstrap.sh`, which
installs ~17 CLI tools (zsh, oh-my-zsh + plugin, direnv, oh-my-posh, ripgrep/fd/
build-essential, neovim, rust, tree-sitter-cli, uv, nvm, node, herdr, aven,
opencode, claude, gh/delta/git-credential-manager, just) via a mix of apt, curl
installer scripts, cargo, and nvm, each gated by an idempotent `has <bin>` check.
There is currently no single place to see at a glance which of those tools are
installed, whether the Stow symlinks are healthy, whether any tool has a newer
version available, or to jump straight into editing `~/.zshrc` (the
most-frequently-edited file, reached today only via its symlink). `pinst`
(`/root/projects/pinst`, currently an empty git repo) is a new Rust project to
build that dashboard.

## Decision
Build `pinst` as a **read-only, detection-focused** ratatui TUI with three tabs
— Overview (tool inventory + live install/version status), Health (Stow symlink
integrity + PATH checks), Upgrades (installed vs. latest version per tool) —
plus a global keybinding that shells out to `$EDITOR` on `~/.zshrc` and other
dotfiles-tracked files. The tool inventory is driven by a `registry.toml` data
file seeded from `bootstrap.sh`'s existing per-step tool list (GUD-001) rather
than a second hardcoded list that can drift. All probing (tool checks, symlink
health, upgrade lookups) runs concurrently via `tokio` so the UI never blocks on
subprocess or network I/O (PAT-001). `pinst` deliberately does **not**
reimplement install/upgrade execution: `bootstrap.sh` and `justfile` already own
that idempotent logic with interactive confirms and `SKIP=`; duplicating it
would double the maintenance surface for no benefit to the stated requirements
(overview, health, upgrades, editor access — all read/detect, not do).

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| cursive or another retained-mode TUI crate instead of ratatui+crossterm | ratatui is the actively maintained tui-rs successor with the widest widget ecosystem; its immediate-mode redraw model fits a dashboard that streams async probe updates far better than a callback-driven retained-mode UI |
| Synchronous `std::process::Command` probing on the main thread | 17 tools × subprocess spawns, plus network calls for upgrade checks, would block the render loop and make the UI feel frozen on startup/refresh — the opposite of "modern interface" |
| Hardcode the tool list as Rust consts | dotfiles' own tool list already changes independently (justfile `packages` var, bootstrap.sh steps, README's "Adding a new package" workflow); an external TOML registry stays in sync without recompiling |
| pinst reimplements install/upgrade actions itself | bootstrap.sh already owns idempotent, interactive, `SKIP=`-aware install logic; reimplementing it in Rust risks drift between two install paths for the same tools |
| Single monolithic `main.rs` | four distinct views each with their own async data source would become unreadable in one file; a modular tree maps 1:1 to the phased build order |

## Consequences
- **DEP-001** ratatui (widgets/layout), **DEP-002** crossterm (terminal backend,
  raw mode, alternate screen, input events), **DEP-003** tokio (async runtime,
  process spawning, channels), **DEP-004** serde + toml (registry
  deserialization), **DEP-005** color-eyre (error handling + panic hook that
  restores the terminal), **DEP-006** `which` crate (PATH lookups), **DEP-007**
  reqwest or ureq (GitHub releases API calls for upgrade detection) become
  project dependencies.
- **RISK-001**: `apt-cache policy` / `cargo search` / GitHub releases API calls
  can be slow or rate-limited (unauthenticated GitHub API ≈60 req/hr) — mitigated
  by an on-disk TTL cache and a manual-refresh keybinding on the Upgrades tab
  rather than auto-run on every launch.
- **RISK-002**: `--version` output formats are heterogeneous and version parsing
  is fragile — mitigated with a per-tool `version_regex` field in the registry
  instead of one generic parser; unparseable output degrades to "unknown"
  rather than crashing.
- **RISK-003**: a panic mid-render can leave the terminal in raw
  mode/alternate-screen — mitigated by installing a color-eyre/panic hook that
  restores the terminal before printing, established in Phase 1 before any
  other view code exists.
- **RISK-004**: Stow health checks must not false-positive on files
  intentionally adopted via `just adopt` — mitigated by only flagging a Stow
  target as "conflict" when it is a plain file that differs from the repo's
  tracked version, not merely "not a symlink".
- **ASSUMPTION-001**: pinst targets the same Ubuntu/WSL + apt environment
  bootstrap.sh targets; no macOS/brew support is in scope.
- **ASSUMPTION-002**: `$EDITOR` is set in the environment (the dotfiles zsh
  config likely sets it); pinst falls back to `vi` if unset.
- **ASSUMPTION-003**: `registry.toml` ships inside the `pinst` repo for v1.
  Whether it should later move into `~/dotfiles` to be a single source of truth
  alongside the justfile `packages` var is deferred — see Open Questions.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Skeleton UI & terminal lifecycle | [phase-01.md](phase-01.md) | Done |
| 2 | Tool registry + async probing + Overview view | [phase-02.md](phase-02.md) | Done |
| 3 | Dotfiles Health view | [phase-03.md](phase-03.md) | Done |
| 4 | Config quick-access / editor launch | [phase-04.md](phase-04.md) | Done |
| 5 | Upgrade detection | [phase-05.md](phase-05.md) | Done |

## Affected Files
- FILE-001: `Cargo.toml` — new crate manifest with all deps
- FILE-002: `registry.toml` — new, tool registry data seeded from bootstrap.sh
- FILE-003: `src/main.rs` — new, entry point + terminal lifecycle + panic hook
- FILE-004: `src/app.rs` — new, App state machine, tab enum, key dispatch
- FILE-005: `src/event.rs` — new, unified event loop (crossterm + tokio mpsc)
- FILE-006: `src/ui/mod.rs` — new, `draw()` dispatch by active tab
- FILE-007: `src/ui/theme.rs` — new, palette/style constants
- FILE-008: `src/ui/statusbar.rs` — new, shared status/hint bar
- FILE-009: `src/ui/overview.rs` — new, Tools/Overview tab widget
- FILE-010: `src/ui/health.rs` — new, Dotfiles Health tab widget
- FILE-011: `src/ui/upgrades.rs` — new, Upgrades tab widget
- FILE-012: `src/registry/mod.rs` — new, `ToolSpec` + `load_registry()`
- FILE-013: `src/probe/mod.rs` — new, `ProbeResult` + `spawn_probe_tasks()`
- FILE-014: `src/probe/version.rs` — new, version string parsing
- FILE-015: `src/health/mod.rs` — new, symlink + PATH health checks
- FILE-016: `src/editor.rs` — new, `launch_editor()` terminal suspend/resume
- FILE-017: `src/upgrade/mod.rs` — new, `UpgradeStrategy` + `UpgradeResult`
- FILE-018: `src/upgrade/strategies.rs` — new, per-strategy latest-version lookups

## Open Questions
- ASSUMPTION-003: should `registry.toml` eventually live in `~/dotfiles` itself
  (single source of truth with the justfile `packages` var), or stay inside the
  `pinst` repo indefinitely? Deferred until the registry has proven itself in v1.
