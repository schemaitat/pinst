# Phase 2 — CLI shell + AI-readiness contract

## Status
Done

## Goal
GOAL-002: Put a clap-driven command surface in front of the core with the
uniform agent contract — `--json` envelope on stdout, human logs on stderr,
stable exit codes, `--dry-run`/`--yes`, no prompting off a TTY — and demote the
TUI to `pinst tui`, so every command added later inherits the contract instead
of inventing one.

## Why this phase exists
The contract has to exist before the first mutating command, not after: if
`install` ships first and `doctor` second, they will disagree about output
shape and exit codes, and an agent cannot be written against a moving target.
Building it on Phase 1's validated domain model means the first two real
commands (`list`, `schema`) exercise the envelope end to end without needing
any execution machinery.

## Steps
- [x] TASK-006: `Cargo.toml` — add DEP-001 clap (derive) and DEP-002 schemars.
- [x] TASK-007: `src/cli/output.rs` — `Envelope<T>` (`schema_version`,
      `command`, `status`, `items`, `errors`), `Status`, the exit-code enum
      (0 success, 1 unexpected failure, 2 usage error, 3 issues/drift found),
      and emitters that put data on stdout and everything human on stderr.
      Why: GUD-002 — an agent piping `--json` must never get progress text
      mixed into the payload, and a distinct exit code for "issues found" lets
      an agent branch without parsing prose.
- [x] TASK-008: `src/cli/mod.rs` — `Cli` with global flags (`--json`,
      `--dry-run`, `--yes`, `--quiet`, `--manifest`), the `Commands` enum
      covering the full eventual surface (later ones stubbed as "not yet
      implemented" with a stable error shape), and TTY detection defaulting to
      non-interactive when stdout is not a terminal (SEC-002, ALT-006).
- [x] TASK-009: `src/main.rs` — dispatch to command handlers and map their
      result to the exit codes from TASK-007; `pinst tui` launches the
      existing dashboard.
- [x] TASK-010: `src/cli/commands/list.rs` + `src/cli/commands/schema.rs` —
      the first two real commands: `list` reports the manifest inventory with
      live probe status, `schema manifest` emits the derived JSON Schema.
      Why: `schema` is the core "easy to extend" affordance (REQ-007) — it
      lets an agent author a manifest entry without reading Rust source.
- [x] TASK-011: `src/cli/output.rs` — tests asserting the envelope shape and
      the exit-code mapping.

## Trade-offs & risks
- Stubbing the not-yet-implemented subcommands is deliberate: the surface an
  agent sees stays stable from this phase on, and each later phase fills a stub
  rather than adding a new verb.
- ASSUMPTION: `--json` implies non-interactive; a prompt in JSON mode would
  deadlock an agent, so confirmation-gated steps in JSON mode fail with a
  remediation hint ("re-run with --yes") instead of asking.
- RISK-007: the TUI moves behind a subcommand here and must still launch.

## Done criteria
- TEST-002: `pinst list --json` and `pinst schema manifest` emit valid JSON
  that parses with stdout clean of log lines; exit codes match the documented
  mapping (including 2 for a usage error); `pinst tui` still launches the
  dashboard.
