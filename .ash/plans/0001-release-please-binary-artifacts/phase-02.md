---
index: "0001"
slug: release-please-binary-artifacts
phase: 2
status: Proposed
---

# Phase 2 — release-please owns the version

## Goal
GOAL-002: Hand versioning, `CHANGELOG.md`, tagging and GitHub release
creation to release-please, driven by the Conventional Commits the repo
already writes, so that `Cargo.toml`'s version is never edited by hand again
and cutting a release is exactly one action: merging the release PR.

## Why this phase exists
It depends on Phase 1 only: release-please's output is a pull request that
rewrites `Cargo.toml`, `Cargo.lock` and `CHANGELOG.md`, and that PR must be
gated by the CI added there before anyone merges it. It is kept separate from
Phase 3 because a release that exists but carries no binary is still a
correct, useful release — the tag and the changelog stand on their own — and
because the two halves fail for completely different reasons (repo
permissions and commit parsing here, cross-compilation there). Debugging them
together would mean re-cutting releases to test a build.

## Steps
- [ ] TASK-003: verify the repo setting ASSUMPTION-001 depends on — Settings
      → Actions → General → Workflow permissions set to read *and write*, and
      "Allow GitHub Actions to create and approve pull requests" enabled.
      Why: without the second checkbox release-please fails to open its PR
      and the failure reads as a generic 403; checking first costs a minute
      and saves an hour. This is a human step, not a code change.
- [ ] TASK-004: `.release-please-manifest.json` — new, `{".": "0.1.0"}`,
      matching `Cargo.toml` today.
      Why: manifest mode needs to be told where the version currently stands;
      without it the first run re-derives a version from tag history that
      does not exist yet.
- [ ] TASK-005: `release-please-config.json` — new: one package at `"."` with
      `"release-type": "rust"`, `"include-component-in-tag": false` (so the
      tag is `v0.2.0`, which `github_latest` in
      `src/core/upgrade/strategies.rs` already parses by stripping the `v`),
      `"draft": true`, and a `changelog-sections` list that keeps `feat`,
      `fix`, `perf` and `docs` visible.
      Why: `release-type: rust` is what makes it update `Cargo.toml` *and*
      `Cargo.lock`; `draft: true` is what closes the RISK-002 window in
      Phase 3, and setting it now means Phase 3 only has to add the publish
      step.
- [ ] TASK-006: `.github/workflows/release.yml` — new workflow on
      `push: [main]`, `permissions: { contents: write, pull-requests: write }`,
      with a single `release-please` job running
      `googleapis/release-please-action@v4` (`id: release`) against the two
      config files, and a final step that prints `toJSON(steps.release.outputs)`.
      Why: the printed outputs are how Phase 3 learns the exact names to
      branch on — `releases_created`/`release_created`/`tag_name` differ
      between manifest and single-package mode, and guessing (RISK-003) means
      a chained job that silently never runs.
- [ ] TASK-007: land a `feat:` or `fix:` commit on `main` and confirm the
      release PR appears with the bumped `Cargo.toml`, the updated
      `Cargo.lock` and a generated `CHANGELOG.md`. Do not merge it yet.
      Why: leaving it open is the cleanest hand-off — Phase 3 merges it once
      the artifact job exists, so the project's first real release is also the
      first end-to-end test of the whole chain.

## Trade-offs & risks
- RISK-003 is the live one here, and TASK-006's output dump exists purely to
  retire it before Phase 3 depends on those names.
- RISK-006: from now on the release PR is itself a PR, so Phase 1's CI runs on
  it. That is intended — the lockfile release-please writes gets the same
  `--locked` check as any other change.
- RISK-007/0.x semantics accepted: the first release is `0.2.0`. Reaching
  `1.0.0` stays a deliberate, separate decision (`Release-As:` or a
  `BREAKING CHANGE:` after 1.0), not a side effect of this plan.
- ASSUMPTION-004: single package at `"."`. If pinst ever becomes a workspace,
  the config gains packages rather than changing shape.
- Deferred: no release-please branch protection, no signed tags, no
  `bootstrap-sha` — the history is short enough that the default scan is fine.

## Done criteria
- TEST-002: a conventional commit merged to `main` produces a release PR that
  bumps `[package] version` in `Cargo.toml`, updates the `pinst` entry in
  `Cargo.lock`, and writes a `CHANGELOG.md` entry attributing the change to
  the right section; the workflow log shows the release-please outputs with
  their exact names.
