# TODO — what is left of plan 0001

Everything implementable is merged or in review. What remains needs rights the
implementing agent does not hold (see `learnings.md`, ISSUE-008).

## Human-gated, in this order

- [ ] **Merge PR #3** — `ci: read release-please's tag output under both names`.
      Must land before #2. Without it the release is tagged while the
      `artifacts` job reads an empty `tag_name`, producing a draft release
      with no assets (ISSUE-006).
      ```sh
      gh pr merge 3 --rebase --delete-branch
      ```
- [ ] **Merge PR #2** — `chore: release main` (0.2.0). This is the release:
      it tags `v0.2.0`, creates the draft, builds the musl binary, attaches
      the tarball + `.sha256` + provenance attestation, then un-drafts.
      ```sh
      gh pr merge 2 --squash
      ```

## Then, verification — closes phases 3 and 4

- [ ] **TASK-014** — watch the `release` run. The `artifacts` job must not be
      skipped; if it is, the `if:` is reading an output name that does not
      exist and the run log's "Show release-please outputs" step says which
      names are real.
- [ ] **TEST-003** — the artifact is reachable and sound:
      ```sh
      curl -fsSLO https://github.com/schemaitat/pinst/releases/latest/download/pinst-x86_64-unknown-linux-musl.tar.gz
      curl -fsSLO https://github.com/schemaitat/pinst/releases/latest/download/pinst-x86_64-unknown-linux-musl.tar.gz.sha256
      sha256sum -c pinst-x86_64-unknown-linux-musl.tar.gz.sha256
      tar -xzf pinst-x86_64-unknown-linux-musl.tar.gz && ./pinst --version   # expect 0.2.0
      ```
- [ ] **TASK-019 / TEST-005** — `pinst update pinst` replaces the running
      binary and reports the new version afterwards.
- [ ] **TEST-004** — `scripts/install.sh` on a machine with no Rust installs
      the released binary; `PINST_BUILD_FROM_SOURCE=1` still compiles.
- [ ] Flip `phase-03.md` and `phase-04.md` to `status: Done`, mirror both in
      the README's `## Phases` table, set the README's own status to `Done`,
      regenerate `.ash/INDEX.md`, and append the closing entries to
      `.ash/CHANGELOG.log`.

## Deliberately not done

- **TASK-015 / TASK-016** stay unticked for good: RISK-004 did not reproduce
  and the change was reverted (`learnings.md`, ISSUE-002). Not forgotten work.
