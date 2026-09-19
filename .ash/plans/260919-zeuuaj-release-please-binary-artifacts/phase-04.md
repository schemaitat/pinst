---
id: 260919-zeuuaj
slug: release-please-binary-artifacts
phase: 4
status: Done
---

# Phase 4 — Consume the artifact

## Goal
GOAL-004: Turn the published asset into the default way pinst arrives on a
machine — `install.sh` downloads it instead of compiling, and the manifest
gains an entry for pinst itself so `pinst update pinst` keeps it current —
then document both.

## Why this phase exists
It depends on Phase 3 alone, and cannot precede it: everything here resolves
`/releases/latest/download/`, so it is only safe to ship once that URL
returns a real, published artifact. Until this phase, the release is a
by-product nobody reads; after it, the release is the distribution channel,
which is why the two are separated — a regression here is a broken install
path, and it should be revertable without touching how releases are cut.

## Steps
- [ ] TASK-015: `src/core/exec/github_release.rs` — extract into a temp
      directory and then `mv` the contents into `dest`, instead of untarring
      straight into `dest`.
      Why: RISK-004 — untarring over a running executable fails with
      `ETXTBSY`, which is exactly what `pinst update pinst` would do.
      `mv` replaces the directory entry rather than the open inode, so it
      succeeds; every other tarball-installed tool gets the same
      all-or-nothing extraction as a side benefit. Keep the existing
      "clean up the staged download even when extraction fails" behavior,
      and keep the `sudo` elevation rule for destinations outside `$HOME`.
- [ ] TASK-016: `src/core/exec/github_release.rs` — unit test covering the
      staged-then-moved action sequence, including the elevation branch.
      Why: this is the one behavioral change in the plan, and the failure it
      prevents only reproduces while pinst is running itself.
- [x] TASK-017: `scripts/install.sh` — try the release asset first:
      `curl -fsSL` the tarball for the host triple from
      `/releases/latest/download/`, verify it against the `.sha256`, install
      it to `$PINST_INSTALL_DIR`, and fall back to the existing
      clone-and-build path when the download fails, when the host is not
      `x86_64` Linux, or when `PINST_BUILD_FROM_SOURCE=1` is set. Add
      `PINST_VERSION` to pin a specific tag.
      Why: REQ-003 and RISK-005 — the fast path removes the Rust toolchain
      from a fresh-machine install entirely, and the retained source path
      means a missing asset degrades to today's behavior instead of leaving
      the machine with nothing.
- [x] TASK-018: `manifest.toml` — add a `[[tool]] name = "pinst"` entry:
      `detect` via `command -v pinst` with `version_cmd = "pinst --version"`,
      `install = { method = "github_release", repo = "schemaitat/pinst",
      asset = "pinst-x86_64-unknown-linux-musl.tar.gz", dest = "$HOME/.local/bin" }`,
      and a `dev` tag.
      Why: this is what CON-001's fixed asset name was for. It makes pinst an
      ordinary managed tool — `pinst list` shows its version, `pinst update`
      offers the upgrade — using the `github_release` upgrade strategy that
      already parses `tag_name` by stripping the leading `v`.
- [x] TASK-019: verify the manifest edit loads and self-update works:
      `pinst list --json` (exit 2 means the entry is invalid),
      `pinst plan pinst --json`, then `pinst update pinst` against the real
      release.
      Why: TASK-015 exists for this exact call; running it is the only
      honest proof.
- [x] TASK-020: `AGENTS.md` — document the release process in a short
      section: conventional commits feed release-please, merging the release
      PR cuts `v<x.y.z>`, the asset name is stable, and the binary is
      self-updating via `pinst update pinst`.
      Why: AGENTS.md is the contract an agent reads before touching this
      repo; "how do I ship this" belongs there next to "how do I extend the
      manifest".
- [x] TASK-021: `README.md` — replace the build-from-source install
      instructions with the download one-liner, keep the source build as the
      documented fallback, and describe the release flow for humans.
      Why: the README currently tells a reader to compile pinst; leaving that
      as the headline install would waste the entire plan.

## Trade-offs & risks
- RISK-004 is retired by TASK-015/TASK-016; until then, `pinst update pinst`
  must not be advertised.
  Superseded: TASK-015 and TASK-016 stay unticked on purpose. RISK-004 did
  not reproduce — GNU tar unlinks before extracting, so untarring over the
  running binary never raises `ETXTBSY` — and the staged-extract change was
  reverted rather than kept as insurance (learnings.md ISSUE-002, LESSON-001).
  They are not forgotten work, and `pinst update pinst` is advertised.
- RISK-005 is mitigated, not eliminated: a corrupt-but-downloadable asset
  still installs. The `.sha256` check is what catches that, so it is part of
  TASK-017 rather than an enhancement.
- ASSUMPTION-002 surfaces here: `install.sh` must detect a non-`x86_64` or
  non-Linux host and fall back rather than downloading an incompatible
  binary.
- Accepted: pinst appearing in its own manifest means `pinst apply` on a
  fresh machine can install a *second* copy at `~/.local/bin` alongside a
  binary invoked from elsewhere. The detect command resolving through `PATH`
  keeps that idempotent in the normal case; an unusual install location is
  the user's to reconcile.
- Deferred: no `pinst self-update` command — the manifest entry covers it
  with no new surface.

## Done criteria
- TEST-004: on a container with no Rust toolchain and no checkout,
  `curl -fsSL .../scripts/install.sh | sh` puts a working `pinst` on `PATH`
  in seconds; `PINST_BUILD_FROM_SOURCE=1` still takes the compile path; a
  deliberately corrupted download aborts instead of installing;
  `pinst doctor --json` runs and reports on the machine.
- TEST-005: `pinst update pinst` replaces the running binary without
  `ETXTBSY`, `pinst --version` afterwards reports the new release, and the
  new unit test covers the staged-extract action sequence.

## Confirmed after the merge
Verified on 2026-09-19 against the published v0.4.0.
- TEST-004 ✓ — `scripts/install.sh` run with no cargo on `PATH` and outside a
  checkout downloaded, verified and installed `pinst 0.4.0`; a deliberately
  corrupted download aborted with "checksum mismatch — refusing to install
  it" and left the install dir empty; `pinst doctor --json` runs and exits 3
  with findings, which is its contract.
- TEST-005 ✓ / TASK-019 ✓ — `pinst update pinst -y` from the installed
  binary replaced itself in place (0.1.0 → 0.4.0, exit 0, no `ETXTBSY`) and
  `pinst --version` afterwards reported `pinst 0.4.0`. The clause about a new
  unit test is void with TASK-015/TASK-016 dropped.
  The stale binary also re-confirmed LESSON-004: 0.1.0 planned
  `sudo tar -C $HOME/.local/bin`, the released 0.4.0 plans it without sudo.
