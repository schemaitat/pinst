---
id: 260920-qcrrqs
slug: macos-support
phase: 4
status: Proposed
---

# Phase 4 — Build and publish the Darwin artifacts

## Goal

**GOAL-004**: make `just dist` produce a correct tarball for either Darwin
target on a Mac, and make the release workflow build and attach both
alongside the existing musl asset — so that
`/releases/latest/download/pinst-aarch64-apple-darwin.tar.gz` resolves.

## Why this phase exists

Phases 1–3 made a macOS pinst worth installing; nothing so far has made one
exist. This is deliberately the fourth slice and not the first: the release
workflow is the one piece of this plan that cannot be tested by running it
locally, and a draft release with a broken or missing asset has burned this
repo before — `v0.3.0` shipped assetless because `just dist target=<triple>`
silently passed a literal string as a positional argument. Building artifacts
only once there is something correct to put in them keeps a failed run here
from being confused with a defect above.

It is separate from Phase 5 for the reason the previous release plan split
build from consume: `install.sh` cannot be tested against an asset that does
not exist yet, and writing both together means the first real test of either
is a release.

## Steps

- [ ] TASK-025: `justfile` (FILE-015) — change `dist`'s native-vs-`cross`
      heuristic from comparing architectures to comparing the OS/vendor
      portion of the triple: same OS and vendor means `rustup target add` plus
      plain `cargo build --target`, and only a genuinely foreign OS falls to
      `cross`.
      Why: this is LESSON-005 read one field further along. The current test
      is `"${target%%-*}" = "${host%%-*}"` — architecture only — so an arm64
      Mac building `x86_64-apple-darwin` fails the test and takes the `cross`
      branch, which has no Apple support and is not installed on the runner.
      Apple's toolchain cross-compiles between its own architectures natively;
      the container is for a foreign OS, which is now the only case left.
- [ ] TASK-026: `justfile` (FILE-015) — replace the GNU-only archive flags
      with a portable path: detect `gtar` and use the reproducible flags when
      it is present, otherwise fall back to plain `tar -czf` with a comment
      recording that the Darwin archives are not byte-reproducible.
      Why: RISK-004 and CON-003. BSD tar rejects `--sort`, `--owner=0`,
      `--group=0` and `--numeric-owner` outright. Reproducibility exists here
      so a locally built artifact can be compared against the released one;
      keeping that property where the tool allows it and stating plainly
      where it does not is better than silently dropping it on one platform
      or adding a `gnu-tar` dependency to every Mac that builds a release.
- [ ] TASK-027: `justfile` (FILE-015) — compute the checksum with
      `sha256sum` when present and `shasum -a 256` otherwise, writing the same
      `<asset>.sha256` file either way.
      Why: CON-003 — macOS has no `sha256sum`. `scripts/install.sh` already
      handles exactly this fork when *verifying*; the producing side must do
      the same or the file it writes will be in whichever format the builder
      happened to have. The two tools' output formats agree, so the artifact
      is identical; only the invocation differs.
- [ ] TASK-028: `.github/workflows/release.yml` (FILE-016) — turn the matrix
      into `{target, runner}` pairs:
      `x86_64-unknown-linux-musl`/`ubuntu-latest`,
      `aarch64-apple-darwin`/`macos-latest`,
      `x86_64-apple-darwin`/`macos-latest`; set
      `runs-on: ${{ matrix.runner }}`.
      Why: CON-002, DEP-002, PAT-003 — the previous release plan shaped this
      as a matrix precisely so a second platform would be rows rather than a
      rewrite, and this is that claim being cashed.
- [ ] TASK-029: `.github/workflows/release.yml` (FILE-016) — make the
      "Install musl toolchain" step conditional on the Linux row.
      Why: it is `sudo apt-get install musl-tools cmake`, which fails
      immediately on a macOS runner. The Darwin rows need no equivalent —
      `aws-lc-rs` builds against the CLT toolchain the runner image already
      carries — so the step gains a condition rather than a macOS counterpart.
- [ ] TASK-030: `.github/workflows/release.yml` (FILE-016) — confirm the
      `just dist ${{ matrix.target }}` invocation, the build-provenance
      attestation and the `gh release upload --clobber` step are all
      per-target and need no change beyond the matrix.
      Why: they interpolate `matrix.target` already, so this is a read rather
      than an edit — but it is the read that catches a hard-coded asset name
      hiding among them, and the positional-argument bug that cost `v0.3.0` a
      release lived in exactly this line.
- [ ] TASK-031: `README.md` (FILE-019) — update the release section: three
      assets, which platforms they serve, and that Intel Macs get a prebuilt
      binary rather than a source build.
      Why: the README currently states the single musl asset as the release
      artifact. Leaving it would make the documented contract disagree with
      the published one, and the release URL shape is what `install.sh` and
      pinst's own `github_release` executor both depend on.

## Trade-offs & risks

- **RISK-004** is handled by detection rather than by requiring `gtar`, which
  means the Darwin tarballs are genuinely less reproducible than the Linux
  one on a runner without it. That is a real, accepted asymmetry, written
  into the recipe as a comment so the next person to compare a local build
  against a release knows why it differs.
- **DEP-002** is where this phase can fail for reasons outside the tree. Both
  Darwin targets are expected to build on `macos-latest`; if
  `x86_64-apple-darwin` needs an SDK flag the runner does not set by default,
  the fallback is to drop that row and revert to ALT-008 (aarch64 only), with
  `install.sh` falling back to a source build for Intel. That fallback is
  named here so it is a prepared retreat rather than a decision taken at 2am
  against a red release.
- ASSUMPTION-004 is why TASK-025 is written the way it is: if `macos-latest`
  is an Intel runner, the two Darwin rows swap which is native and which is
  the same-OS cross. That task's heuristic is written on OS and vendor, not
  architecture, so it is correct either way — which is the point of fixing it
  that way rather than special casing arm64.
- **RISK-006** applies from this phase onward: the release only becomes
  atomic once all three assets have uploaded. Nothing new is needed to hold
  that — `publish` already waits on the whole `artifacts` job — but a failed
  Darwin row now means an undownloadable draft, and that is the correct
  failure to have.
- Accepted: no `universal2` asset (ALT-009). Two named assets keep the
  mapping from `uname -m` to a filename mechanical, which is what Phase 5
  needs.
- Deferred: `install.sh` still refuses Darwin after this phase. The assets
  exist and are downloadable by hand; teaching the installer to pick one is
  Phase 5.

## Done criteria

**Provable in the tree:**

- **TEST-022**: `actionlint` accepts `release.yml`, and the matrix expands to
  three `{target, runner}` pairs with the musl-toolchain step conditioned on
  the Linux row.
- **TEST-023**: `just dist x86_64-unknown-linux-musl` on Linux still produces
  a byte-identical tarball and `.sha256` to the one produced before this
  phase. The portability changes must be a no-op for the existing target.
- **TEST-024**: the checksum and archive forks are exercised, not just
  written — run the recipe with `sha256sum` and with `gtar` shadowed by
  `PATH` so the fallback branch is the one that executes (LESSON-029), and
  confirm the resulting `.sha256` still verifies with `shasum -a 256 -c`.
- **TEST-025**: `just qc` is green.

**Confirmed after landing (needs DEP-002, a real release run):**

- **TEST-026**: a release run produces three assets plus three `.sha256`
  files, all attached to the draft before `publish` un-drafts it.
- **TEST-027**: `curl -fsSL .../releases/latest/download/pinst-aarch64-apple-darwin.tar.gz`
  resolves and its checksum verifies.
- **TEST-028**: `just dist aarch64-apple-darwin` and
  `just dist x86_64-apple-darwin` both succeed on a Mac, taking the native
  and the same-OS-cross branch of TASK-025 respectively (DEP-004).

## Confirmed after the merge

*(To be filled in when the first release carrying Darwin assets completes —
record which rows built, which branch of TASK-025 each took, and whether
`gtar` was present on the runner.)*
