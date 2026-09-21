---
id: 260920-qcrrqs
slug: macos-support
phase: 2
status: Done
---

# Phase 2 — Homebrew as an install method

## Goal

**GOAL-002**: add `brew` as an eighth install method — executor, plan step,
upgrade check and version normalization — and prove it end to end on a small
set of real manifest entries, so that Phase 3's remaining thirteen overrides
are data entry rather than design.

## Why this phase exists

Phase 1 can express *that* a tool installs differently on macOS but has no
method to express it *with*: every override it can write today reuses a
Linux-shaped method. This phase supplies the missing half, and stops
deliberately short of converting the whole manifest. Converting all fourteen
apt tools in the same phase would mean the first time `brew_latest` is
exercised is against fourteen formulae at once, and a version-comparison bug
(RISK-003) would present as fourteen spurious upgrades with no clean case to
compare against.

It cannot merge into Phase 3 for the same reason in reverse: `Install::Brew`
must exist and be tested before thirteen manifest entries depend on it.

## Steps

- [x] TASK-009: `src/core/manifest.rs` (FILE-003) — add
      `Install::Brew { formulae: Vec<String>, #[serde(default)] cask: bool }`
      and `UpgradeSpec::Brew { formula: String }`, plus the `method_name`
      (`"brew"`) and `upgrade_spec` arms. `is_unpinnable` stays false for
      brew.
      Why: additive variants, so every existing manifest and every existing
      `--json` consumer is unaffected (CON-005, GUD-002). `cask` is a boolean
      rather than a separate method because the command differs by one word
      and nothing else. Brew is not unpinnable: unlike a piped installer it
      reports an exact installed version, which is what `is_unpinnable`
      actually distinguishes.
- [x] TASK-010: `src/core/exec/brew.rs` (FILE-007) — implement `Executor`:
      `install` emits `brew install [--cask] <formulae>`; `upgrade` emits
      `brew upgrade [--cask] <formulae>`. Never prefix `sudo`.
      Why: Homebrew refuses to run as root and exits non-zero with a message
      about it (RISK-002, SEC-002). The `Runner` is the single place pinst
      touches the system and applies no elevation of its own, so this holds as
      long as the emitted command text does not ask for it.
- [x] TASK-011: `src/core/exec/mod.rs` (FILE-008) — `mod brew;` and the
      `Install::Brew { .. } => Box::new(brew::Brew)` arm in `executor_for`.
- [x] TASK-012: `src/core/plan.rs` (FILE-010) and `src/core/engine.rs`
      (FILE-009) — add `StepKind::BrewUpdate`, and in both
      `build_install_plan` and the upgrade plan emit a single
      `brew update` step with id `brew:update` immediately before the first
      brew step, mirroring the existing `apt_update_added` latch.
      Why: a new `StepKind` variant rather than renaming `AptUpdate` to
      something neutral. Renaming would change a value that already appears in
      `pinst plan --json` output, and the schema contract is additive-only
      (CON-005). Two variants also keep the rendered plan honest about which
      package manager is being refreshed.
- [x] TASK-013: `src/core/upgrade/strategies.rs` (FILE-011) — add
      `brew_latest(formula)` shelling out to
      `brew info --json=v2 --formula <formula>`, reading
      `.formulae[0].versions.stable`, and returning `None` on any failure.
      Why: `--json=v2` is the stable machine interface; parsing `brew info`'s
      human output would break on the next brew release. Returning `None`
      rather than erroring matches every other strategy in this file — an
      upgrade report is advisory and one unreachable source must not hide the
      others.
- [x] TASK-014: `src/core/upgrade/mod.rs` (FILE-012) — extend the existing
      version normalization so a brew revision suffix (`14.1.0_1`) compares
      equal to the upstream version it decorates, the way apt epoch/revision
      decoration already does.
      Why: RISK-003. Without it every brew tool reports an upgrade that
      upgrading does not resolve, and a permanently-firing check is one that
      gets ignored and then deleted (LESSON-011). The apt path proves the
      shape of the fix is already understood here; this is the second caller,
      not a new idea.
- [x] TASK-015: `manifest.toml` (FILE-013) — add the `homebrew` tool:
      `detect = { command = "command -v brew", bin = "brew", version_cmd = "brew --version" }`,
      installed by `curl_script` against Homebrew's official installer with
      `NONINTERACTIVE=1`, and marked `unsupported` on Linux via
      `[tool.platform.linux]`.
      Why: brew has to be installable before anything can be installed with
      it, and a manifest entry is how pinst expresses that — the same way
      `nvm` precedes `node`. Marking it unsupported on Linux uses the
      mechanism from Phase 1 in the direction that is easy to forget: the
      override block is not a macOS-only feature.
- [x] TASK-016: `manifest.toml` (FILE-013) — give exactly three tools a
      `[tool.platform.macos]` block using `method = "brew"`, each
      `requires = ["homebrew"]`: `ripgrep`, `direnv` and `delta` (formula
      `git-delta`).
      Why: three, not fourteen. `ripgrep` is the trivial case (same name
      everywhere); `direnv` exercises a tool whose Linux `upgrade` is derived
      rather than explicit; `delta` is the case where the formula name
      differs from the tool name, which is exactly where a derived
      `UpgradeSpec::Brew` would pick the wrong string. Between them they cover
      every way TASK-009's `upgrade_spec` arm can be wrong, on a set small
      enough to read.

## Trade-offs & risks

- **RISK-002** is mitigated by construction but not enforced: nothing stops a
  future edit to the brew executor, or to `engine.rs`'s privilege handling,
  from prefixing `sudo`. TEST-008 asserts the absence of `sudo` in the
  emitted command so the constraint has a test rather than only a comment.
- **RISK-003** is the one this phase is most likely to get subtly wrong,
  because the failure is not an error — it is a report that is merely always
  wrong. TEST-009 pins the specific shape (`_1` suffix) rather than testing
  the comparison abstractly.
- **DEP-001** arrives here as a manifest entry, which means a macOS plan now
  has a first step that needs sudo once (ASSUMPTION-001). That is visible in
  `pinst plan --platform macos` before anything runs, which is the property
  that makes it acceptable.
- Accepted: `brew update` is emitted once per run, before the first brew
  step, even though Homebrew auto-updates on its own schedule and the step is
  therefore often redundant. Matching the apt latch exactly is worth more than
  saving the seconds — one mechanism, one place to reason about.
- Accepted: `brew info --json=v2` is a subprocess per formula, as
  `apt-cache policy` already is. Batching all formulae into one `brew info`
  call would be faster and would lose the per-tool `None`-on-failure
  isolation that this file is built around.
- Deferred: the remaining eleven apt tools, `neovim`'s Linux asset, pinst's
  own asset, and the config changes. All of them are Phase 3, and none of
  them can be written before this phase's `brew` method exists.

## Done criteria

- **TEST-007**: unit tests for the brew executor: `install` on a two-formula
  tool emits `brew install <a> <b>`; `cask = true` emits `brew install --cask`;
  `upgrade` emits `brew upgrade`.
- **TEST-008**: a test asserting no command emitted by the brew executor
  contains `sudo` (RISK-002, SEC-002).
- **TEST-009**: a version-comparison test asserting installed `14.1.0_1`
  against latest `14.1.0` reports *no* upgrade available, sitting beside the
  existing apt-epoch case (RISK-003).
- **TEST-010**: `upgrade_spec()` on the `delta` entry resolved for macOS
  yields `UpgradeSpec::Brew { formula: "git-delta" }` — the derived spec
  follows the formula name, not the tool name.
- **TEST-011**: a plan-shape test asserting `brew:update` appears exactly
  once and strictly before the first brew install step, mirroring the existing
  `apt_update_is_emitted_once_before_the_first_apt_install` test.
- **TEST-012**: `pinst plan --platform macos --json` from Linux shows
  `homebrew` ordered before `ripgrep`, `direnv` and `delta`, and shows
  `homebrew` as `tool.unsupported.homebrew` under `--platform linux`.
- **TEST-013**: `brew_latest` returns `None` when `brew` is not on `PATH` —
  arranged by running the check with a `PATH` that excludes it rather than by
  assuming the Linux dev box has no brew (LESSON-029: the documented
  degradation is the test specification).
- **TEST-014**: `just qc` is green, and `pinst schema output` still
  round-trips with the new `StepKind` variant present.
