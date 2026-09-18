# Phase 2 — Tool registry + async probing + Overview view

## Status
Done

## Goal
GOAL-002: Ship a TOML tool registry seeded from `bootstrap.sh`'s existing tool
list, probe all registered tools concurrently for installed/version status,
and render the result as the Overview tab — the core "overview of all tools
installed" requirement (REQ-002).

## Why this phase exists
This is the first phase that touches real dotfiles state, and it is the
dependency every other data view (Health, Upgrades) leans on: Health
cross-checks PATH presence against these same probe results (Phase 3), and
Upgrades dispatches per-tool lookups keyed by the same registry entries
(Phase 5). Building it right after the Phase 1 skeleton — before Health or
Upgrades — means those later phases can reuse its `ToolSpec`/`ProbeResult`
types instead of inventing parallel ones.

## Steps
- [x] TASK-006: `registry.toml` — seed one entry per `bootstrap.sh` step
      (zsh, oh-my-zsh, direnv, oh-my-posh, ripgrep/fd/build-essential, neovim,
      rust, tree-sitter-cli, uv, nvm, node, herdr, aven, opencode, claude,
      gh/delta/git-credential-manager, just), each with `name`, `check_cmd`,
      `version_cmd`, `version_regex`, `install_method`
      (`apt|cargo|curl_script|nvm`).
      Why: GUD-001 — reuses bootstrap.sh's own `has()` list instead of a
      separately maintained one that can drift.
- [x] TASK-007: `src/registry/mod.rs` — `ToolSpec` struct (serde
      `Deserialize`) + `load_registry(path) -> Vec<ToolSpec>`.
- [x] TASK-008: `src/probe/mod.rs` — `ProbeResult { tool, installed: bool,
      version: Option<String> }`; `spawn_probe_tasks(registry, tx:
      UnboundedSender<Event::Probe>)` spawning one tokio task per `ToolSpec`
      running `check_cmd` then `version_cmd` via `tokio::process::Command`
      (PAT-001).
- [x] TASK-009: `src/probe/version.rs` — apply each tool's `version_regex` to
      raw `--version` output to extract a normalized version string;
      unparseable output yields `None` rather than erroring (RISK-002).
- [x] TASK-010: `src/ui/overview.rs` — `Table`/`List` widget rendering
      registry tools with live status icons (probing… / ✓ installed+version /
      ✗ missing), updated as `Event::Probe` messages drain each tick; wire in
      as the Overview tab in `app.rs` and into `event.rs`'s select loop.

## Trade-offs & risks
- RISK-002 (fragile version parsing) is accepted as "unknown" rather than
  solved generically — acceptable because the Overview view's primary signal
  is installed/missing, with version as a secondary detail.
- Probing all ~17 tools concurrently on every launch is accepted for now (no
  caching in this phase, unlike Phase 5's upgrade lookups) since local `has`/
  `--version` checks are cheap and don't hit rate limits — only Phase 5's
  network calls need a TTL cache (RISK-001).

## Done criteria
- TEST-003: Overview tab lists all tools from `registry.toml` with correct
  installed/missing status matching manual `command -v <tool>` checks for a
  sample of installed and not-installed tools.
- TEST-004: Startup probing of all ~17 tools completes and renders without
  blocking input (UI remains responsive — e.g. tab switch works while probes
  are still in flight).
