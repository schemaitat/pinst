---
index: "0001"
slug: release-please-binary-artifacts
status: In Progress
created: 2026-09-19
updated: 2026-09-19
areas: [ci, distribution, manifest]
summary: Automate releases with release-please and attach a self-contained Linux binary tarball to every GitHub release.
files_touched: [.github/workflows/ci.yml, .github/workflows/release.yml, release-please-config.json, .release-please-manifest.json, justfile, .gitignore, scripts/install.sh, manifest.toml, src/core/exec/github_release.rs, README.md, AGENTS.md]
---

# Release-please-driven releases that ship a prebuilt pinst binary

## Context
pinst's entire premise is "one self-contained binary that gets downloaded onto
a fresh machine" (Cargo.toml's release profile says so in a comment), but no
such download exists. The repo has no `.github/` directory at all: no CI, no
release automation, no published artifact. `scripts/install.sh` — the
documented fresh-machine entry point — compiles from source, and installs
Rust first if the box has none, which is a multi-minute build on a machine
that was supposed to need nothing (REQ-003). Versioning is equally manual:
`Cargo.toml` has said `0.1.0` since the first commit even though the history
already follows Conventional Commits (`feat(agents):`, `fix(configs):`,
`chore:`), enforced by `.agents/skills/conventional-commits` (PAT-002). The
work here is to close both gaps at once: every merge to `main` feeds
release-please, and every release it cuts carries a ready-to-run binary
(REQ-001, REQ-002, REQ-004).

## Decision
A single `.github/workflows/release.yml` holds **two chained jobs**. The first
runs `googleapis/release-please-action@v4` in manifest mode with
`release-type: rust`, so it maintains `Cargo.toml`, `Cargo.lock` and
`CHANGELOG.md`, opens a release PR, and — when that PR merges — creates the
`v<x.y.z>` tag and a **draft** GitHub release. The second job runs only when
the first reports a release, builds `x86_64-unknown-linux-musl` through
`cross`, uploads `pinst-x86_64-unknown-linux-musl.tar.gz` plus a `.sha256`
sidecar and a build-provenance attestation, and then flips the release out of
draft.

Three properties shaped this. **Chaining jobs rather than listening for
`release: published`** avoids needing a PAT: releases created with the default
`GITHUB_TOKEN` do not trigger further workflows, and adding a long-lived token
to dodge that is a secret this repo does not otherwise need (SEC-001,
ALT-003). **musl, not glibc**, because a binary that inherits the runner's
glibc baseline is exactly the "works on my distro" artifact this project
exists to avoid (ALT-004). **A fixed, version-free asset name**, because
pinst's own `github_release` executor resolves
`https://github.com/<repo>/releases/latest/download/<asset>` — a constant path
with no version in it (CON-001) — so naming the asset this way is what later
lets pinst install and upgrade *itself* from the manifest, and lets
`install.sh` fetch a binary with one `curl` and no Rust. The build itself
lives in a `just dist` recipe that CI calls, keeping one answer to "how is an
artifact built" the same way `install.sh` is the one answer to "how does pinst
get installed" (PAT-001, CON-004).

CI comes first, as its own phase. release-please tags whatever reaches `main`,
so automating releases without a gate in front of them just automates shipping
broken binaries.

## Alternatives Considered
| Option | Why rejected |
|--------|-------------|
| ALT-001: `cargo-dist` to generate the release workflow and installers | It wants to own versioning and tagging too, which collides head-on with release-please; it generates far more workflow than one crate with one target needs, and its upstream stewardship is uncertain. The requirement is one small workflow, not a generator |
| ALT-002: Cut releases by hand (`gh release create` from a maintainer machine, or a `just release` recipe) | Leaves the version bump and changelog hand-written — precisely what REQ-002 removes — and produces an artifact whose provenance is one laptop |
| ALT-003: A separate workflow keyed on `release: published` | Releases created with the default `GITHUB_TOKEN` do not trigger workflows, so this needs a PAT or GitHub App token: a long-lived secret bought for nothing that job chaining does not already give (SEC-001) |
| ALT-004: Build `x86_64-unknown-linux-gnu` on the runner | The artifact's job is to run on a machine that has nothing; dynamic linking pins it to the builder's glibc and breaks on anything older than the runner image |
| ALT-005: Version-stamped asset names (`pinst-0.2.0-x86_64-unknown-linux-musl.tar.gz`) | Unfetchable via `/releases/latest/download/<asset>`, which is the only URL shape pinst's own `github_release` executor knows (CON-001). The version is already in the tag, the release title and the changelog |
| ALT-006: Publish a bare binary instead of a tarball | The `github_release` executor extracts with `tar`; a tarball is what pinst can already consume, and it carries the executable bit through the download |
| ALT-007: Distribute via crates.io or an apt PPA | crates.io install requires a Rust toolchain on the target machine, defeating REQ-003; a PPA is a signing-and-hosting project of its own |
| ALT-008: `actions/upload-artifact` | Expires after 90 days, requires auth to download, and has no stable URL — it is a CI convenience, not a distribution channel |

## Consequences
- **DEP-001** `googleapis/release-please-action@v4`; **DEP-002** `cross` for
  the musl build; **DEP-003** `dtolnay/rust-toolchain`, `Swatinem/rust-cache`,
  `extractions/setup-just`; **DEP-004**
  `actions/attest-build-provenance@v2` (SEC-002); **DEP-005** the `gh` CLI,
  preinstalled on GitHub runners, for asset upload and un-drafting.
- **RISK-001**: `reqwest` pulls `rustls`, whose default crypto provider here is
  `aws-lc-rs` (confirmed via `cargo tree -i aws-lc-rs`), and that needs `cmake`
  plus a C toolchain to build for musl. Mitigated by building inside `cross`'s
  musl image, which ships both; if it still fails, switching reqwest to the
  `ring` provider is a one-line Cargo.toml change and remains cheaper than
  giving up static linking. This is validated in Phase 3 before anything
  depends on the asset existing.
- **RISK-002**: `/releases/latest/download/<asset>` 404s between release
  creation and asset upload — a window in which a fresh-machine install would
  fail. Mitigated by creating the release as a draft and publishing it only
  after the assets land.
- **RISK-003**: release-please v4's output names differ between manifest and
  single-package modes; the chained job's `if:` condition must be wired
  against the outputs the action actually emits, not remembered ones.
- **RISK-004**: once pinst can install pinst, `tar`-ing over the running
  binary yields `ETXTBSY`. Mitigated by extracting to a temp dir and `mv`-ing
  into place, which is atomic and benefits every tarball-installed tool.
- **RISK-005**: making `install.sh` prefer the release asset means a broken or
  missing asset breaks fresh-machine installs. Mitigated by keeping the source
  build as an automatic fallback plus an explicit `PINST_BUILD_FROM_SOURCE=1`.
- **RISK-006**: 0.x semantics — while the version is below `1.0.0`, `feat:`
  bumps the minor and a breaking change bumps the minor too. The first release
  cut by this plan will be `0.2.0`, not `1.0.0`; going 1.0 is a deliberate,
  separate act.
- **ASSUMPTION-001**: the repo is `schemaitat/pinst` and its Actions settings
  permit workflows to write contents *and* to create pull requests — without
  "Allow GitHub Actions to create and approve pull requests", release-please
  cannot open its PR and the whole chain silently does nothing.
- **ASSUMPTION-002**: Linux x86_64 is the only target that matters today,
  consistent with the manifest's apt/Ubuntu assumption. The workflow is shaped
  as a matrix so a second target is a row, not a rewrite.
- **ASSUMPTION-003**: `main` is the release branch.
- **ASSUMPTION-004**: pinst stays a single crate at the repo root, so
  release-please's manifest has exactly one package at `"."`.

## Phases
| # | Phase | File | Status |
|---|-------|------|--------|
| 1 | A CI gate on main | [phase-01.md](phase-01.md) | Done |
| 2 | release-please owns the version | [phase-02.md](phase-02.md) | Proposed |
| 3 | Build and attach the binary | [phase-03.md](phase-03.md) | Proposed |
| 4 | Consume the artifact | [phase-04.md](phase-04.md) | Proposed |

## Affected Files
- FILE-001: `.github/workflows/ci.yml` — new: fmt, clippy, test, locked build
- FILE-002: `README.md` — CI badge, download-based install, release process
- FILE-003: `release-please-config.json` — new: manifest-mode config, `release-type: rust`, draft releases
- FILE-004: `.release-please-manifest.json` — new: current version pin (`{".": "0.1.0"}`)
- FILE-005: `.github/workflows/release.yml` — new: release-please job + chained artifact job
- FILE-006: `justfile` — `dist` recipe producing the release tarball and checksum
- FILE-007: `.gitignore` — ignore `/dist`
- FILE-008: `CHANGELOG.md` — generated and maintained by release-please
- FILE-009: `src/core/exec/github_release.rs` — extract to a temp dir, then `mv` into `dest`
- FILE-010: `scripts/install.sh` — download the release asset first, build from source as fallback
- FILE-011: `manifest.toml` — a `[[tool]]` entry for pinst itself
- FILE-012: `AGENTS.md` — how a release happens and what the artifact is called

## Open Questions
None. ASSUMPTION-001 is the one that needs a human to check a repo setting
before Phase 2 can work; the rest are accepted as stated.
