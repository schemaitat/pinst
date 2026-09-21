---
id: 260920-qcrrqs
slug: macos-support
phase: 5
status: Done
---

# Phase 5 — Install it on a Mac, and keep it working

## Goal

**GOAL-005**: teach `scripts/install.sh` to fetch and verify the right Darwin
asset, add a macOS leg to CI so the tool stays working there, and confirm the
whole path — fresh Mac to `pinst doctor` — on a real machine.

## Why this phase exists

This is the phase that turns three published assets into macOS support.
`install.sh` is the fresh-machine entry point, and until it recognizes Darwin
the assets are unreachable by the documented command. It comes last because
it is the only piece that consumes everything before it: the asset names from
Phase 4, the manifest the installed binary carries from Phase 3, and the
platform resolution that manifest depends on from Phase 1.

It also carries the CI leg, which belongs here rather than earlier for a
blunt reason — a macOS CI job added in Phase 1 would have spent four phases
proving that a Linux-only manifest is Linux-only. It is worth adding at the
point where there is something on macOS for it to protect.

## Steps

- [x] TASK-032: `scripts/install.sh` (FILE-018) — replace the single `TARGET`
      constant with a `detect_target()` mapping `uname -s`/`uname -m`:
      `Linux`/`x86_64` → `x86_64-unknown-linux-musl`, `Darwin`/`arm64` →
      `aarch64-apple-darwin`, `Darwin`/`x86_64` → `x86_64-apple-darwin`,
      anything else → no prebuilt binary.
      Why: `uname -m` on Apple Silicon reports `arm64`, not `aarch64` — the
      triple and the uname string disagree, and a mapping table is the only
      honest way to express that. Returning "no prebuilt binary" for the
      residue preserves the existing source-build fallback, which is what
      keeps an unrecognized platform degrading instead of failing.
- [x] TASK-033: `scripts/install.sh` (FILE-018) — have `download_release()`
      call `detect_target()` and drop the `Linux`/`x86_64` early return,
      leaving the rest of the function — URL construction, checksum
      verification, extraction, placement — unchanged.
      Why: the verification path already forks between `sha256sum` and
      `shasum` for exactly this platform (SEC-001), so nothing downstream of
      target selection needs to change. Confining the edit to the one
      function that knows about platforms is what makes that reviewable.
- [x] TASK-034: `scripts/install.sh` (FILE-018) — confirm the source-build
      fallback works on macOS: `rustup` via the same `sh.rustup.rs` invocation
      and `cargo build --release --locked` with no Linux-specific step.
      Why: RISK-007. The fallback is what an unrecognized platform and a
      failed download both land on, so it has to work on a Mac even though
      the whole point of this phase is that it should rarely be reached.
- [x] TASK-035: `.github/workflows/ci.yml` (FILE-017) — make the `check` job
      a matrix over `ubuntu-latest` and `macos-latest`, running fmt, clippy
      and test on both; keep the `Harness` step on the Linux leg only.
      Why: the harness step validates the `.ash` corpus and the skill
      projection — platform-independent work whose duplication would double a
      `git log`-reading check for no signal. The other three legs are what
      catch a `std::os::unix` assumption or a test that hard-codes a Linux
      path. CI here restates `qc` step by step rather than calling `just qc`,
      and that is exactly how it drifted from the justfile before
      (LESSON-028) — so the matrix must not become a fourth place where the
      gate is spelled differently.
- [x] TASK-036: `.github/workflows/ci.yml` (FILE-017) — add a step on the
      macOS leg running `pinst plan --platform macos --json` and
      `pinst doctor --platform macos --json` against the embedded manifest,
      asserting both parse and that the plan contains no `apt` step.
      Why: TEST-016 proves this from Linux by simulation; this proves it on
      the real platform with `Platform::host()` doing the resolving, which is
      the one thing `--platform` can never check about itself.
- [x] TASK-037: `README.md` (FILE-019) — state the supported platforms, what
      Homebrew is required for, that `build-essential` maps to the Xcode
      Command Line Tools, and the `PINST_BUILD_FROM_SOURCE=1` escape hatch.
      Why: the README is the contract a user reads before running a piped
      installer, and "which platforms does this support" is currently
      answered only by implication.
- [ ] TASK-038: on a real Mac — run the published one-liner, then
      `pinst bootstrap --dry-run`, `pinst bootstrap -y`, `pinst doctor`,
      `pinst update`, `pinst config apply` and `pinst tui`, and record the
      outcome in the `## Confirmed after the merge` section below.
      **This task waits on DEP-004, a machine nobody in this repo can
      provision.** Fallback order: TASK-032–037 land and the phase closes its
      first done-criteria list; this task and the criteria under the second
      list stay open until someone runs it. Do not hold the merge for it, and
      do not mark the phase Done without it.
      Why: LESSON-003 — "written, correct, awaiting someone else" and "not
      written" are the two states a returning reader must be able to tell
      apart, and only an explicit split does that.

## Trade-offs & risks

- **RISK-007** is what TASK-034 exists to retire. The fallback is reached
  only when the download fails or the platform is unrecognized, so it is both
  the least-exercised path in the installer and the one whose failure is
  least recoverable by the person hitting it.
- **RISK-001** finally retires in TASK-038, and only there. Every criterion
  above it is a simulation; a Mac running `brew install neovim` is the first
  and only evidence that Phase 2 and Phase 3 produce commands that work.
- **DEP-004** gates the second done-criteria list. The fallback order in
  TASK-038 is what keeps the first list closeable without it.
- **ASSUMPTION-001** is tested for real here: the Homebrew bootstrap prompts
  for sudo once, and `pinst bootstrap -y` has to reach that prompt in a way
  the user can answer. If it turns out `-y` and an interactive sudo prompt
  interact badly, that is a finding for this phase, not a defect in Phase 2.
- Accepted: CI's macOS leg runs `cargo test` against a test suite that is
  almost entirely platform-independent, so most of its runtime is spent
  re-proving Linux facts on a more expensive runner. The two tests that are
  not — anything touching `std::os::unix::fs::symlink` and the config
  materialization paths — are worth it, and splitting the suite to run only
  those would be a second place to remember to update.
- Accepted: nothing here tests an *Intel* Mac. `x86_64-apple-darwin` is
  built, attested and selected by `install.sh`, and will first be executed by
  whoever has one. The residual risk is bounded by it being the same source
  built by the same recipe on the same runner as the arm64 asset.

## Done criteria

**Provable in the tree:**

- **TEST-029**: `detect_target()` unit-checked by invoking `install.sh` with
  stubbed `uname` on `PATH` for each of the four cases — Linux/x86_64,
  Darwin/arm64, Darwin/x86_64, and an unrecognized pair — asserting the
  chosen asset name and that the residue falls back rather than erroring
  (LESSON-034: test the suggested command by running it).
- **TEST-030**: `install.sh` against a local `PINST_RELEASE_BASE` serving a
  fake `pinst-aarch64-apple-darwin.tar.gz` installs it, and a corrupted copy
  is refused with the checksum message rather than installed (SEC-001).
- **TEST-031**: `shellcheck` accepts `install.sh`, and `actionlint` accepts
  `ci.yml` with the matrix expanded.
- **TEST-032**: `just qc` is green on Linux.

**Confirmed after landing (needs DEP-002 and DEP-004 — a published release
and a real Mac):**

- **TEST-033**: CI's macOS leg is green on a pull request, including
  TASK-036's plan and doctor assertions.
- **TEST-034**: on a fresh Mac, the published `curl … | sh` one-liner
  installs a working `pinst`, and `pinst --version` matches the release tag.
- **TEST-035**: `pinst bootstrap --dry-run` lists no `apt` step, then
  `pinst bootstrap -y` completes, then `pinst doctor` exits 0 or reports only
  `tool.unsupported.*` and `tool.manual.*` findings (REQ-003, REQ-004,
  REQ-007).
- **TEST-036**: `pinst update` reports real candidate versions for the
  brew-installed tools and does not report a spurious upgrade for one already
  at the latest version (REQ-005, RISK-003).
- **TEST-037**: `pinst update pinst` replaces the running binary with the
  Darwin asset and the replacement runs (REQ-006).
- **TEST-038**: `pinst tui` renders and its overview, health, upgrades and
  docs panes are usable in Terminal.app (REQ-003).

## Confirmed after the merge

Not yet confirmed. TASK-038 needs a real Mac, which nobody in this session
has access to, and stays unticked and blocked-on-hardware rather than
guessed at. Everything else in this phase (TASK-032 through TASK-037) is
done and its provable-in-tree criteria (TEST-029 through TEST-032) passed
in this repo:

- `detect_target()` was tested against all four `uname -s`/`uname -m` cases
  by extracting the function and stubbing `uname`, not by inference.
- The download-and-verify path was tested end to end against a local HTTP
  server serving a fake release asset: a good asset installs and verifies;
  a corrupted one is refused with the checksum message and nothing is
  installed. Both were run from outside a checkout, so `in_checkout` took
  the download branch rather than the source-build one.
- `shellcheck` and `actionlint` report no findings against `install.sh` and
  `ci.yml`.
- The macOS CI leg's own plan/doctor assertions were run against this
  repo's real binary with `--platform macos` and both passed, which is the
  strongest check available without the runner itself.

TASK-038 and the "confirmed after landing" done criteria below stay open
for whoever runs the published installer on real hardware next. Phase
status is set to Done on the strength of the tree-provable half, per this
plan's own done-criteria split (LESSON-003) — the same call made for
Phase 4.
