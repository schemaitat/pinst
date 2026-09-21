---
id: 260920-qcrrqs
slug: macos-support
status: Proposed
created: 2026-09-20
updated: 2026-09-21
areas: [manifest, exec, release, configs]
summary: Make pinst a macOS tool as well as a Linux one — platform-aware manifest, a brew install method, and released Darwin binaries.
files_touched: [src/core/platform.rs, src/core/manifest.rs, src/core/graph.rs, src/core/doctor.rs, src/core/exec/brew.rs, src/core/exec/mod.rs, src/core/engine.rs, src/core/plan.rs, src/core/upgrade/strategies.rs, src/cli/mod.rs, manifest.toml, configs/zsh/.zshrc, justfile, .github/workflows/release.yml, .github/workflows/ci.yml, scripts/install.sh, README.md]
---

# macOS support: a released Darwin binary, and every command working behind it

## Context

pinst is Linux-only by construction, and in three separate layers. The
manifest installs fourteen of its twenty-two tools with `apt` (CON-001); the
release workflow builds exactly one target, `x86_64-unknown-linux-musl`, a
decision recorded as ASSUMPTION-002 in
`260919-zeuuaj-release-please-binary-artifacts`; and `scripts/install.sh`
refuses anything that is not `Linux`/`x86_64`, falling back to a source build
that then fails on the manifest anyway. The tool is wanted on a Mac, which
means all three layers have to move together — a Darwin binary that installs
and then cannot install anything is not macOS support.

The requirements are the whole surface, not the binary alone: the release
must produce macOS artifacts (REQ-001), `install.sh` must fetch and verify one
(REQ-002), every command must run (REQ-003), installs must work through some
macOS package manager (REQ-004), upgrade checks must answer (REQ-005),
`pinst update pinst` must replace the right asset (REQ-006), and the config
packages must apply (REQ-007).

Three constraints shape everything below. Apple targets cannot be
cross-compiled from a Linux runner — they need the Apple SDK — so the release
matrix has to grow a macOS runner rather than a target triple (CON-002). The
macOS userland is BSD, so `tar --sort` and `sha256sum` (both load-bearing in
`just dist`) are not present (CON-003). And Homebrew's prefix depends on the
architecture — `/opt/homebrew` on Apple Silicon, `/usr/local` on Intel — so
no path to a brew-installed binary can be hard-coded (CON-004). A fourth,
quieter one: `pinst --json` is the machine contract this project is built
around, so every schema change here has to be additive (CON-005).

## Decision

Platform becomes a first-class value inside pinst rather than an ambient
property of the process. A `Platform` enum is resolved once at startup — from
`target_os`, overridable by `--platform` and `PINST_PLATFORM` — and threaded
into manifest resolution, planning and doctor as a parameter (GUD-001). Each
`[[tool]]` keeps a single identity and gains an optional
`[tool.platform.macos]` block that overrides `detect`, `install`, `upgrade`,
`requires` and `post_install`, or declares the tool `unsupported` with a note.
Tool identity, tags, `requires` edges, profiles and docs pages stay
single-sourced; only the parts that actually differ are written twice.

A tool whose base `install` method is Linux-only (`apt`) and which carries no
macOS override is *not supported* on macOS: it is filtered out of selection
and doctor reports it as `tool.unsupported.<name>` at info severity, carrying
whatever remediation the manifest gave. A partial macOS manifest therefore
stays loadable and usable, and its gaps are visible instead of fatal
(ALT-004, ALT-005).

The macOS package manager is Homebrew, added as an eighth install method
behind the existing `Executor` trait (PAT-001) — `Install::Brew` and
`UpgradeSpec::Brew`, additive variants alongside `Apt` (GUD-002). Homebrew
itself becomes a manifest entry, installed by its own `curl_script`, that the
brew-installed tools require.

The release grows two matrix rows, `aarch64-apple-darwin` and
`x86_64-apple-darwin`, both built on a `macos-latest` runner, with the
existing musl row pinned to `ubuntu-latest` (PAT-003). `just dist` stays the
one answer to "how is an artifact built" (GUD-003), which means fixing its
host-vs-target heuristic — today it compares *architectures* and would send an
arm64 host building `x86_64-apple-darwin` down the `cross` branch, which has
no Apple support — and making its archive and checksum steps work under BSD
userland.

## Alternatives Considered

| Option | Why rejected |
|--------|-------------|
| ALT-001: `platforms = ["linux"]` plus a duplicate tool entry per platform | Every shared field — `summary`, `tags`, `requires`, the docs page key — is written twice and drifts independently. The duplication is on the parts that do *not* differ, which is exactly backwards. |
| ALT-002: a second embedded `manifest.macos.toml` | Cleanest separation, but duplicates all four `[[config]]` blocks and both `[profile.*]` blocks, neither of which is platform-specific. Two whole manifests to keep in step to express fourteen differing lines. |
| ALT-003: `cfg!(target_os = "macos")` at each call site | Untestable from CI on Linux, and untestable from a Mac for the Linux half — the behaviour could only be checked by running on both. LESSON-018: ambient state a test wants to override should be a parameter. It would also make `pinst plan --platform macos` impossible, and that is the only way this plan gets developed without a Mac. |
| ALT-004: reject a manifest at load time when a tool has no macOS path | Makes every partial manifest unloadable on a Mac, including a user's own `~/.config/pinst/manifest.toml` override, and turns a routine gap into exit 2 from `pinst list`. |
| ALT-005: express "no macOS path" as the existing `Install::Manual` | Conflates "a human has to do this step" with "this tool does not apply to this machine". `manual` warns and tells you to go install it; `unsupported` should be information about a tool that is legitimately absent. |
| ALT-006: cross-compile Darwin targets from Linux via `cross`/osxcross | Needs the Apple SDK, whose redistribution is a licensing question this project should not be answering. LESSON-005 also cuts the other way here: `cross` earns its container for a foreign *architecture*, not a foreign OS it cannot link for at all. |
| ALT-007: MacPorts or Nix as the macOS package manager | Homebrew is what a Mac that is not already a Nix machine has; MacPorts needs sudo for everything and has a smaller set of the formulae this manifest wants. Nix would replace the manifest rather than back it. |
| ALT-008: ship `aarch64-apple-darwin` only and let Intel build from source | Rejected by the decision on target coverage: an Intel Mac falling back to a rustup install plus a full release build is a twenty-minute first run, for one extra matrix row. |
| ALT-009: a `universal2` fat binary via `lipo` | One asset instead of two, but it needs a `lipo` step that exists in no other row of the matrix, doubles the download for every user, and makes the asset name unpredictable from `uname -m` — which is precisely how `install.sh` picks. |

## Consequences

What gets easier: a third platform is a `Platform` variant, an executor and a
matrix row. What gets harder: the manifest now has two readings, and a change
to a tool's Linux install no longer implies anything about its macOS one — the
`[tool.platform.macos]` block has to be checked by hand, because nothing can
check it mechanically.

- **DEP-001**: Homebrew, installed by the new `homebrew` manifest entry via
  `NONINTERACTIVE=1` and its official installer. Its own bootstrap needs
  sudo once, to create the prefix.
- **DEP-002**: GitHub's `macos-latest` runner (arm64) with both Darwin rustup
  targets added. Both are buildable on it; `x86_64-apple-darwin` is a
  same-OS cross that the Apple toolchain handles natively (LESSON-005).
- **DEP-003**: Xcode Command Line Tools for `cc`/`make`, which
  `build-essential` maps onto. Not installable non-interactively in the
  general case, so it is declared `unsupported` with `xcode-select --install`
  as its note rather than automated.
- **DEP-004**: a real Mac to confirm the end-to-end run on. Everything up to
  and including the artifact is provable on Linux via `--platform macos`;
  the last mile is not (see Phase 5's split done criteria, LESSON-003).
- **RISK-001**: `--platform macos` renders a *plan*, never a system, so it
  proves the manifest resolves and orders correctly and proves nothing about
  whether `brew install ripgrep` succeeds. It is a strong check against the
  wrong class of bug and no check at all against the other.
- **RISK-002**: Homebrew refuses to run as root, and several of these tools
  are installed by piped scripts that may or may not want sudo. A macOS plan
  that emits `sudo brew` would fail every step. The brew executor never
  elevates (SEC-002), which is a decision that has to survive future edits to
  `engine.rs`'s privilege handling.
- **RISK-003**: brew version strings carry revision suffixes (`14.1.0_1`)
  the way apt candidates carry epochs. `core::upgrade`'s comparison already
  normalizes apt decoration; brew needs the same treatment or every
  brew-installed tool reports a spurious upgrade forever, which is the shape
  of finding that gets switched off (LESSON-011).
- **RISK-004**: the reproducible-archive flags in `just dist`
  (`--sort=name --owner=0 --group=0 --numeric-owner`) are GNU tar spellings
  that BSD tar rejects outright. The failure is at archive time in CI, which
  is loud — but the silent variant, an archive that builds with the flags
  quietly ignored, is why the flags get verified rather than assumed.
- **RISK-005**: `configs/zsh/.zshrc` hard-codes
  `/opt/nvim-linux-x86_64/bin` on `PATH`. Harmless on a Mac (a missing
  `PATH` entry costs nothing) but it is a Linux path in a file applied to
  both, and it is adjacent to the `brew shellenv` line macOS needs.
- **RISK-006**: three release assets means a release is atomic only once all
  three have uploaded. The existing `publish` job already gates un-drafting
  on the whole `artifacts` job, so a failed Darwin row leaves an
  undownloadable draft rather than a `latest` with a hole in it — an
  inherited property that now guards three assets instead of one.
- **RISK-007**: `install.sh`'s source-build fallback is the least-exercised
  path in the installer and is what an unrecognized platform or a failed
  download lands on. If it is broken on macOS the symptom is a fresh Mac with
  no pinst and a compiler error — the worst place to discover anything, which
  is the same reasoning `build.rs` records about stale embedded configs.
- **ASSUMPTION-001**: the macOS target is a developer machine where the user
  can sudo. The Homebrew bootstrap requires it once.
- **ASSUMPTION-002**: `zsh` is already the login shell on macOS, so the
  `chsh` post-install step is a no-op there. Its `skip_if` already expresses
  this; what changes is that on macOS `chsh` additionally requires the shell
  to be listed in `/etc/shells`.
- **ASSUMPTION-003**: every tool in the manifest that is not `apt`-installed
  (`cargo`, `curl_script`, `nvm`, `git_clone`, `shell`) works unmodified on
  macOS. This is checked per tool in Phase 3, not assumed wholesale — it is
  the assumption most likely to be wrong for at least one entry.
- **ASSUMPTION-004**: `macos-latest` remains an arm64 runner. If it moved
  back to Intel, the two Darwin rows swap which one is native; both are still
  built, so this is a build-time cost, not a correctness issue.

## Phases

| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | Platform becomes a value pinst can reason about | [phase-01.md](phase-01.md) | Done |
| 2 | Homebrew as an install method | [phase-02.md](phase-02.md) | Done |
| 3 | A manifest and configs that cover macOS | [phase-03.md](phase-03.md) | Done |
| 4 | Build and publish the Darwin artifacts | [phase-04.md](phase-04.md) | Done |
| 5 | Install it on a Mac, and keep it working | [phase-05.md](phase-05.md) | Proposed |

## Affected Files

- **FILE-001**: `src/core/platform.rs` *(new)* — the `Platform` enum, its
  resolution from `target_os`/`PINST_PLATFORM`, and which install methods
  each platform admits.
- **FILE-002**: `src/core/mod.rs` — register the `platform` module.
- **FILE-003**: `src/core/manifest.rs` — `PlatformOverride`, the
  `[tool.platform.<name>]` map on `Tool`, `Tool::resolve(platform)`,
  `Install::Brew`, `UpgradeSpec::Brew`, and `method_name`/`is_unpinnable`
  arms for both.
- **FILE-004**: `src/core/graph.rs` — resolve and filter by platform inside
  `select`, so unsupported tools never reach a plan.
- **FILE-005**: `src/core/doctor.rs` — the `tool.unsupported.<name>` finding.
- **FILE-006**: `src/cli/mod.rs` — the global `--platform` flag, plumbed
  through `Ctx`.
- **FILE-007**: `src/core/exec/brew.rs` *(new)* — the Homebrew executor.
- **FILE-008**: `src/core/exec/mod.rs` — register `brew`, add its
  `executor_for` arm.
- **FILE-009**: `src/core/engine.rs` — emit one `brew update` before the
  first brew install, mirroring the existing `apt-get update` handling.
- **FILE-010**: `src/core/plan.rs` — a `StepKind::BrewUpdate` variant.
- **FILE-011**: `src/core/upgrade/strategies.rs` — `brew_latest` via
  `brew info --json=v2`.
- **FILE-012**: `src/core/upgrade/mod.rs` — normalize brew revision suffixes
  alongside the existing apt epoch handling.
- **FILE-013**: `manifest.toml` — the `homebrew` entry, and a
  `[tool.platform.macos]` block on every tool that needs one.
- **FILE-014**: `configs/zsh/.zshrc` — `brew shellenv`, and the Linux-only
  nvim path made conditional.
- **FILE-015**: `justfile` — `dist` fixed for same-OS cross builds and BSD
  userland.
- **FILE-016**: `.github/workflows/release.yml` — matrix rows and per-target
  runners.
- **FILE-017**: `.github/workflows/ci.yml` — a macOS leg of the gate.
- **FILE-018**: `scripts/install.sh` — Darwin target detection and asset
  selection.
- **FILE-019**: `README.md` — the `brew` method row, the platform override
  block, and the supported-platform statement.
- **FILE-020**: `docs/tools/homebrew.toml` *(new)* — the docs page for the
  new manifest entry.

## Open Questions

None. The three decisions that shaped this plan — per-tool platform override
blocks over duplicated entries or a second manifest (ALT-001, ALT-002), both
Darwin architectures over aarch64 alone (ALT-008), and skip-and-report over
load-time rejection for uncovered tools (ALT-004) — were settled before it was
written. ASSUMPTION-003 is the one that implementation may still falsify, and
Phase 3 is where it gets tested per tool rather than accepted.
