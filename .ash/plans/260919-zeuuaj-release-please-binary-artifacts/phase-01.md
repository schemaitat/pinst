---
id: 260919-zeuuaj
slug: release-please-binary-artifacts
phase: 1
status: Done
---

# Phase 1 — A CI gate on main

## Goal
GOAL-001: Put the checks `just qc` already runs — `cargo fmt --check`,
`cargo clippy -D warnings`, `cargo test` — in front of every push and pull
request, so that from here on "merged to `main`" means "built and passed",
which is the precondition for letting automation tag and publish whatever
lands there.

## Why this phase exists
This phase has no predecessor. It must not be folded into Phase 2: the moment
release-please is wired up, merging to `main` starts producing tags and
releases, and a repo with zero CI would be automating the publication of
unverified binaries. It is also the phase that proves the Actions setup works
at all (runner image, Rust 1.98/edition 2024, cache) against cheap, fast jobs
rather than against a release that is awkward to retract.

## Steps
- [x] TASK-001: `.github/workflows/ci.yml` — new workflow on
      `push: [main]` and `pull_request`, with `permissions: contents: read`.
      One `check` job on `ubuntu-latest`: `actions/checkout@v4`,
      `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`,
      `Swatinem/rust-cache@v2`, then `cargo fmt --check`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`.
      Why: mirror the `qc` recipe's order — it is already ordered to fail
      fastest — and use `--locked` so a stale `Cargo.lock` fails here rather
      than inside a release build; release-please will be editing that
      lockfile from Phase 2 on.
- [x] TASK-002: `README.md` — add the CI status badge next to the title.
      Why: the badge is the only place a reader learns the gate exists;
      the rest of the release story gets documented in Phase 4, once there
      is something to document.

## Trade-offs & risks
- Deferred: no `cargo audit`/`cargo deny`, no MSRV job, no matrix over
  toolchains. One crate, one platform (ASSUMPTION-002) — a second job earns
  its keep when there is a second target.
- Accepted: `--locked` makes a dependency bump a deliberate commit. That is
  the intent; it keeps the lockfile that ships in the release honest.
- GUD-001: this phase changes no behavior, no output shape and no exit code.
  If anything under `src/` needs touching to get CI green, that is a bug this
  phase found, not scope it invented.

## Done criteria
- TEST-001: a pull request against `main` runs the workflow and reports green
  for fmt, clippy (warnings as errors), test, all with `--locked`; a
  deliberately mis-formatted commit pushed to a scratch branch turns it red.
