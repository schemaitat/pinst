# Phase 3 — Install/update execution engine

## Status
Done

## Goal
GOAL-003: Make pinst actually install and update tools — an `Executor` per
install method behind one trait, a dependency-ordered `Plan` that `--dry-run`
prints verbatim, idempotent skip-if-installed, per-step failure isolation, and
confirmation gates on privileged steps — replacing `bootstrap.sh`'s job.

## Why this phase exists
This is the decision reversal made real, and it must come after the CLI
contract so that the riskiest surface in the whole tool inherits `--dry-run`,
`--yes`, and the JSON envelope from day one rather than being retrofitted. It
comes before configs (Phase 4) because the tools are the dependency-ordered
half of the problem: the config half has no ordering constraints and a much
smaller blast radius, so proving the plan→apply machinery here first is the
cheaper failure.

## Steps
- [x] TASK-012: `src/core/plan.rs` — `Plan`, `Step`, `StepKind`, and
      `StepOutcome` (`Skipped`/`Ran`/`Failed`/`WouldRun`), all serializable so
      `--dry-run --json` and a real run share one shape (PAT-002).
- [x] TASK-013: `src/core/exec/mod.rs` — the `Executor` trait plus a command
      runner that classifies each command's privilege (plain, sudo,
      `curl | sh`, writes outside `$HOME`), honors dry-run by recording
      instead of spawning, and refuses confirmation-gated steps unless `--yes`
      or an interactive confirm authorizes them.
      Why: RISK-001/SEC-002 — the gate belongs in the one place every executor
      funnels through, not duplicated per method where it can be forgotten.
- [x] TASK-014: `src/core/exec/{apt,cargo,curl_script,github_release,nvm,git_clone}.rs`
      — one executor per install method, each implementing detect/install/
      upgrade for its method only (GUD-003). The release-tarball case that
      puts neovim in `/opt` downloads natively (reqwest is already a
      dependency) and extracts with the system `tar`, elevating only when the
      destination is outside `$HOME` — so DEP-005 was not needed.
- [x] TASK-015: `src/core/engine.rs` — build a `Plan` from the manifest plus
      current probe state, order it with Phase 1's topological sort, and
      execute with per-step isolation and a final failure summary.
      Why: GUD-001 — `bootstrap.sh` earned this behavior; one failing step
      (e.g. `gh` needing an apt repo that is not configured) must not abort
      the other sixteen.
- [x] TASK-016: `src/cli/commands/{install,update,plan}.rs` — fill the Phase 2
      stubs: `install [tools...]`, `update [tools...]`, and `plan` printing
      the ordered plan without executing anything.
- [x] TASK-017: `src/core/engine.rs` — tests for plan ordering against the
      manifest's real constraints, idempotency (a second run is all
      `Skipped`), and failure isolation (one failed step, the rest still run).

## Trade-offs & risks
- RISK-001 is concentrated here. Accepted mitigations: dry-run parity by
  construction, privilege gating in the shared runner, and never running a
  confirmation-gated step in JSON mode without `--yes`.
- RISK-006: `curl | sh` installers cannot be pinned or verified; the executor
  runs them as the upstream documents and records them as unpinnable rather
  than implying a guarantee it cannot make.
- Deferred: no rollback/uninstall path. Installs are additive and idempotent;
  undoing an install is out of scope for this plan.

## Done criteria
- TEST-003: `pinst plan` and `pinst install --dry-run` emit the same ordered
  step list; a real `install` run on an already-provisioned machine reports
  every step `Skipped` (idempotency); an injected failing step is isolated and
  surfaces in the summary while later steps still run; privileged steps are
  refused without `--yes` in JSON mode.
