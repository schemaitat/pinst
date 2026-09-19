---
id: 260919-zeuuaj
slug: release-please-binary-artifacts
phase: 3
status: Done
---

# Phase 3 — Build and attach the binary

## Goal
GOAL-003: Make every release carry
`pinst-x86_64-unknown-linux-musl.tar.gz` — a statically linked binary built
with the tuned release profile — together with a `sha256` sidecar and a build
provenance attestation, published only once the assets are actually in place.

## Why this phase exists
It depends on Phase 2 alone: the job added here is chained off the
release-please job's outputs, which do not exist until that job does, and its
`if:` condition is written against the names Phase 2's output dump revealed.
Keeping it out of Phase 2 also isolates the single most likely failure in this
plan — musl cross-compilation with `aws-lc-rs` in the dependency tree
(RISK-001) — in a phase where the only thing at stake is an asset, not the
tag or the changelog.

## Steps
- [x] TASK-008: `justfile` — add a `dist` recipe taking a target (default
      `x86_64-unknown-linux-musl`): build with `cross build --release
      --locked --target {{target}}` (falling back to `cargo` when the target
      is the host), then stage `target/{{target}}/release/pinst` into
      `dist/` as `pinst-{{target}}.tar.gz` with a `.sha256` next to it.
      Why: PAT-001/CON-004 — the workflow must not be the only place that
      knows how an artifact is made. `install.sh` is already the one answer
      to "how does pinst get installed"; this is the one answer to "how is a
      release artifact built", reproducible on a laptop with one command.
- [x] TASK-009: `.gitignore` — add `/dist`.
      Why: TASK-008 writes build output into the tree; it must never be
      committable.
- [x] TASK-010: validate RISK-001 before wiring any CI to it — run
      `just dist` locally (or on a scratch branch job) and confirm
      `aws-lc-rs` builds for musl inside the `cross` image. If it does not,
      switch `reqwest` to the `ring` crypto provider in `Cargo.toml` and
      re-run; only if *that* fails, fall back to
      `x86_64-unknown-linux-gnu` on `ubuntu-22.04` and record the
      regression against ALT-004 in this file.
      Why: every later step assumes the artifact can be produced at all.
      Finding out during a real release means an empty release that has
      already been tagged.
- [x] TASK-011: `.github/workflows/release.yml` — add an `artifacts` job:
      `needs: release-please`, `if:` the release output confirmed in
      TASK-006, `permissions: { contents: write, id-token: write,
      attestations: write }`, and a `strategy.matrix.target` holding the
      single musl triple. Steps: checkout at the release tag, Rust toolchain
      with that target, `Swatinem/rust-cache@v2`,
      `extractions/setup-just@v3`, install `cross`, `just dist
      target=${{ matrix.target }}`, then `gh release upload <tag>
      dist/pinst-<target>.tar.gz dist/pinst-<target>.tar.gz.sha256
      --clobber`.
      Why: a matrix with one row is what makes adding `aarch64` a row rather
      than a rewrite (ASSUMPTION-002); `--clobber` makes a re-run of a failed
      job idempotent, matching the same property pinst demands of itself.
- [x] TASK-012: `.github/workflows/release.yml` — add
      `actions/attest-build-provenance@v2` over the tarball in the same job.
      Why: SEC-002. This repo's own doctor flags `curl | sh` installers as
      unpinnable because they cannot be verified; shipping an unverifiable
      artifact of its own would be the same sin, and attestation plus a
      checksum is the cheap end of fixing it.
- [x] TASK-013: `.github/workflows/release.yml` — add a final `publish` job,
      `needs: artifacts`, running `gh release edit <tag> --draft=false`.
      Why: RISK-002 — `/releases/latest/download/<asset>` is the URL both
      `install.sh` and pinst's `github_release` executor resolve, and it must
      never resolve to a release whose assets have not landed. Publishing
      last makes the release atomic from a consumer's point of view.
- [x] TASK-014: merge the release PR left open by TASK-007 and verify the
      whole chain end to end against the real release.
      Why: the first release is the only cheap opportunity to test this while
      nothing depends on it yet.

## Trade-offs & risks
- RISK-001 is retired by TASK-010 or the plan changes target; it is
  deliberately the first thing this phase does.
- RISK-002 is retired by TASK-013. The residual is a failed `artifacts` job
  leaving a draft release and a tag with no assets — visible, recoverable by
  re-running the job, and invisible to `latest`, which is the point.
- CON-003 accepted: `dist` must not override the tuned release profile
  (`opt-level = "z"`, `lto`, `strip`), so the recipe passes no profile flags
  of its own.
- Deferred: no macOS or aarch64 target, no `.deb`, no Homebrew tap, no
  minisign/cosign signature beyond the GitHub attestation.
- Accepted: `cross` needs Docker on a developer machine, so `just dist` for a
  non-host target only works where Docker is available; the host-target
  fallback keeps the recipe usable everywhere else.

## Done criteria
- TEST-003: the published release carries
  `pinst-x86_64-unknown-linux-musl.tar.gz` and its `.sha256`;
  `curl -fsSL https://github.com/schemaitat/pinst/releases/latest/download/pinst-x86_64-unknown-linux-musl.tar.gz`
  downloads it; the checksum matches; `tar -xzf` yields an executable `pinst`
  at the archive root; `ldd` reports "not a dynamic executable";
  `./pinst --version` prints the release version, and `./pinst list --json`
  works on a machine with no Rust and no checkout.
- TEST-006: `just dist target=x86_64-unknown-linux-musl` on a developer
  machine produces an archive with the same name and the same layout as the
  one CI uploaded.
  TEST-006 ✓ — exercised while implementing TASK-010: `just dist` builds the
  musl tarball on a developer machine with plain cargo, which is why the
  `cross` branch survives only for a foreign architecture.

## Confirmed after the merge
Per LESSON-003 these could not be proved in the working tree; they were
verified on 2026-09-19 against the published v0.4.0.
- TEST-003 ✓ — `pinst-x86_64-unknown-linux-musl.tar.gz` and its `.sha256`
  download from `/releases/latest/download/`, `sha256sum -c` passes, `ldd`
  reports "statically linked", `./pinst --version` prints `pinst 0.4.0`, and
  `./pinst list --json` runs outside any checkout.
- TASK-014 ✓ — the `artifacts` job was not skipped (the release run for #7
  succeeded in 1m42s) and the release is published, un-drafted, with both
  assets attached.
