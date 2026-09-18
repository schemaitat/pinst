# Phase 5 — Upgrade detection

## Status
Done

## Goal
GOAL-005: Detect, per tool, whether a newer version is available than what's
installed, using a lookup strategy dispatched by the tool's install method
(apt/cargo/curl-script/nvm/github-release) — the "possible upgrades"
requirement (REQ-005).

## Why this phase exists
Upgrade detection needs both a known "current version" (from Phase 2's probe
results) and a stable terminal-suspend/resume mechanism already proven safe
under real subprocess use (Phase 4's editor launch) before adding the highest-
risk I/O of the whole plan: outbound network calls that can be slow, rate-
limited, or offline (RISK-001). It is ordered last because it's the most
optional-feeling requirement relative to Overview/Health/editor access, and
because its cache layer (TASK-018) benefits from the app's event loop and
async patterns already being exercised by three prior phases.

## Steps
- [x] TASK-016: `src/upgrade/mod.rs` — `UpgradeStrategy` enum `{Apt, Cargo,
      GithubRelease { repo }, Nvm, Unsupported}` mapped from each `ToolSpec`'s
      `install_method` field (`registry.toml`, TASK-006); `UpgradeResult {
      tool, current, latest, upgrade_available }`.
- [x] TASK-017: `src/upgrade/strategies.rs` — per-strategy latest-version
      lookup: `Apt` via `apt-cache policy <pkg>` parsing the `Candidate` line;
      `Cargo` via `cargo search <crate> --limit 1` parsing the version;
      `GithubRelease` via the GitHub releases API (DEP-007 reqwest/ureq);
      `Nvm` via nvm's remote LTS listing. Each returns `Option<String>`
      latest, `None` on failure rather than erroring the whole view
      (RISK-001).
- [x] TASK-018: on-disk TTL cache (JSON file under the XDG cache dir) for
      upgrade lookups, keyed by tool + strategy, invalidated after e.g. 24h or
      on a manual refresh keybinding.
      Why: RISK-001 — avoids hitting GitHub's ~60 req/hr unauthenticated rate
      limit and avoids slow network calls on every launch.
- [x] TASK-019: `src/ui/upgrades.rs` — render tools with `upgrade_available =
      true` highlighted, current vs. latest version columns, manual refresh
      (`r`) keybinding triggering a re-run of this phase's lookups; wire in as
      the Upgrades tab.

## Trade-offs & risks
- RISK-001 is mitigated (not eliminated) by the TTL cache: a cold cache still
  pays the full network cost on first refresh. Accepted because upgrade
  checking is inherently network-bound and the requirement only asks to
  "show" upgrades, not guarantee sub-second refresh.
- `Unsupported` strategy (tools installed via a bespoke curl script with no
  queryable "latest version" source, e.g. some one-off installers) surfaces as
  "unknown" in the UI rather than blocking the view — consistent with
  Phase 2's RISK-002 handling of unparseable data.
- This phase detects upgrades only; it does not execute them (per the plan's
  read-only/detection-focused Decision) — running an upgrade remains a manual
  step (e.g. re-running the relevant `bootstrap.sh` step, optionally with
  `SKIP=` for everything else).

## Done criteria
- TEST-007: Upgrades tab correctly shows "upgrade available" for a tool with
  an intentionally old cached version vs. a mocked/real latest lookup, and
  gracefully shows "unknown" (not a crash) when network access is unavailable.
