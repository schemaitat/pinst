---
id: 260918-lmmebj
slug: self-contained-toolchain-cli
phase: 4
status: Done
---

# Phase 4 — Embedded configs + native linking

## Goal
GOAL-004: Absorb the dotfiles tree into this repo under `configs/`, embed it
into the binary, and lay it into `$HOME` with pinst's own linking logic —
link-to-source-tree when the repo is present, materialize embedded bytes when
only the binary is — with timestamped conflict backups, drift detection,
adopt, and minimal templating. This is what retires GNU Stow and the
`~/dotfiles` folder.

## Why this phase exists
It follows the install engine because the config layer reuses Phase 3's
plan→apply machinery and privilege gating for `$HOME` writes rather than
inventing a second mutation path, and because a config that depends on a tool
(`.zshrc` needs oh-my-zsh present to be useful) is only meaningful once the
engine can install that tool. Splitting it out from Phase 3 keeps the two
riskiest subsystems — privileged installers and `$HOME` file mutation —
separately testable.

## Steps
- [x] TASK-018: `configs/**` — move the dotfiles tree in (nvim, zsh, herdr,
      git), excluding herdr's live runtime files (`*.sock`, `*.log`,
      `session.json`) and anything secret-shaped (SEC-001).
      Why: those runtime files physically sit inside the herdr package dir
      because `$HOME/.config/herdr` is a directory-level symlink — the current
      health walker already had to learn to skip them.
- [x] TASK-019: `src/core/template.rs` — `${VAR}` substitution over configs
      marked `template = true`, resolving from `~/.config/pinst/values.toml`
      then the environment, failing loudly on an unresolved variable.
      Why: CON-004 — `.gitconfig` hardcodes a git identity and `.zshrc` still
      carries a stale macOS path; baking either into a bootstrap binary would
      misconfigure every new machine.
- [x] TASK-020: `src/core/configs.rs` — embed `configs/` with DEP-003
      include_dir; resolve the source tree (`PINST_SOURCE` or a detected repo
      checkout) to choose link vs materialize; build a config `Plan`; back
      conflicting plain files up to `<file>.pre-pinst.<timestamp>`; detect
      drift by comparing on-disk bytes against source/embedded; implement
      adopt writing on-disk changes back into the source tree when available.
      Why: GUD-001 — the timestamped backup is exactly what `bootstrap.sh`'s
      stow-conflict retry did, and it is the behavior that makes re-running
      safe on a machine that already has a stock `.zshrc`.
- [x] TASK-021: `src/cli/commands/config.rs` — fill the Phase 2 stub with
      `config status|apply|adopt|diff`, all honoring `--json`/`--dry-run`.
- [x] TASK-022: `src/core/configs.rs` — tests against a temporary `$HOME`
      covering link mode, materialize mode, conflict backup, drift detection,
      adopt round-trip, templating, and a guard test that fails if a
      secret-shaped path ever enters `configs/` (RISK-004).

## Trade-offs & risks
- RISK-002: changing a config still requires a rebuild for the *embedded*
  copy; link mode plus adopt keeps the dev loop fast, and the embedded copy is
  refreshed at build time from the same tree, so they cannot silently diverge.
- RISK-003: pinst now owns conflict semantics that Stow used to own. Accepted
  because backup-never-clobber is a strictly safer default than Stow's refuse-
  and-abort, and drift detection is something Stow never offered.
- ASSUMPTION-003: `lazy-lock.json` and friends are treated as ordinary configs,
  matching how the dotfiles repo tracks them today.
- SEC-001: `~/.zshrc.secrets` stays an untracked runtime concern — `.zshrc`
  continues to source it if present, and it is never embedded.

## Done criteria
- TEST-004: against a temporary `$HOME`, `config apply` produces symlinks in
  link mode and real files in materialize mode; an existing conflicting file is
  backed up rather than clobbered; a hand-edited file is reported as drift and
  `adopt` round-trips it into the source tree; a templated config renders with
  values and fails loudly on a missing variable; the secret-exclusion guard
  test passes.
