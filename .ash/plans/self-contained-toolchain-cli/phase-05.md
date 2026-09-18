# Phase 5 — Doctor + apply/sync convergence

## Status
Done

## Goal
GOAL-005: Compose the tool state from Phase 3 and the config state from Phase 4
into one machine-readable `doctor` — findings with stable ids, severities, and
the exact remediation command — plus `apply`, the single idempotent command
that converges a machine, and rewire the TUI's health view onto the same
findings.

## Why this phase exists
Doctor is definitionally the join of the two halves, so it cannot precede
either; and `apply` is only meaningful once both halves can mutate. Putting
them here rather than folding them into Phase 4 keeps "converge the machine"
as one reviewable slice with its own exit-code semantics, and it is the phase
that finally gives the TUI a reason to read core state instead of its own
bespoke health module.

## Steps
- [x] TASK-023: `src/core/doctor.rs` — a `Finding` type (stable `id`,
      `severity`, human `message`, machine-actionable `remediation`, and a
      `fixable` flag) plus the checks composing tool probe state, config drift,
      and unpinnable-installer reporting (RISK-006).
      Why: the stable id and remediation string are what make findings
      actionable by an agent instead of merely readable.
- [x] TASK-024: `src/cli/commands/doctor.rs` — fill the Phase 2 stub: report
      findings, exit 3 when any are present, and apply the fixable subset under
      `--fix` (still honoring `--dry-run`/`--yes`).
- [x] TASK-025: `src/cli/commands/apply.rs` — full convergence in one command:
      install missing tools, apply configs, then re-run doctor and report what
      remains.
      Why: REQ-006 — "keeping the stack in sync" should be one command an
      agent or a cron job can run unattended.
- [x] TASK-026: `src/ui/health.rs` + `src/app.rs` — rewire the TUI health view
      onto doctor findings, retiring the bespoke `src/health/` logic now that
      core owns it.
- [x] TASK-027: `src/core/doctor.rs` — tests: findings raised against a
      deliberately broken environment, `--fix` repairing the fixable subset,
      and exit code 3 vs 0.

## Trade-offs & risks
- Only the safe subset is `--fix`-able: re-linking a config, re-materializing a
  missing file, installing a missing tool. Anything privileged or destructive
  (`chsh`, replacing a conflicting file) stays a reported finding with a
  remediation command the operator runs deliberately (SEC-002).
- RISK-005 first becomes visible here: a machine still linked into
  `~/dotfiles` will show drift findings; that is correct and is what the Phase
  6 cutover resolves.
- Deferred: no historical tracking of findings over time; each run is a
  point-in-time report.

## Done criteria
- TEST-005: doctor raises the expected findings against a deliberately broken
  temporary environment (missing tool, drifted config, broken link), exits 3
  with findings and 0 when clean, `--fix` repairs exactly the fixable subset
  and leaves the rest reported; the TUI health tab renders the same findings.
