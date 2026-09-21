---
id: 260920-zqapye
slug: installable-harness
phase: 3
status: Proposed
---

# Phase 3 — `pinst harness uninstall`

## Goal
Remove exactly what install wrote — no more, and never a file pinst did not put
there — by teaching the executor its first destructive action and driving it
strictly from the receipt.

## Why this phase exists
It is separated from Phase 2 because it is the only phase in this plan that can
destroy something a person wrote, and bundling it into the installer would mean
reviewing "make six symlinks" and "delete six paths" in one diff, at one level
of attention. `Action::Remove` is new to a codebase whose every existing action
is additive or moves a file aside — `Backup` exists precisely so that nothing
has to be deleted — so it deserves its own commit and its own tests (RISK-001).

It has to follow Phase 2 rather than precede it: the receipt is the input, and
without one, uninstall would have to infer what it owns by comparing the target
against the source. That inference is wrong in exactly the dangerous direction —
a personal skill whose name collides with one of ours would look like ours.

## Steps

- [ ] TASK-017: `src/core/plan.rs` — `Action::Remove { path: PathBuf }`, with
      `describe` rendering `remove <path>` and `privilege` returning
      `Privilege::User`; `src/core/exec/mod.rs` — the `Runner` arm, using
      `remove_file` for a symlink or file and `remove_dir_all` only for a
      directory, and returning `Ok(())` when the path is already gone.
      **Why the already-gone case is not an error:** uninstall run twice is a
      normal thing to do, and a second run failing on a path the first removed
      would push people to `rm -rf` instead.
- [ ] TASK-018: `src/core/harness/install/plan.rs` — `build_uninstall_plan`
      taking the receipt: one step per recorded entry, `Skipped` when the path
      is gone, `Blocked` when the path is no longer what install left (a real
      file where a link was recorded, or a copy whose content no longer matches
      the source) unless `--force`, `Remove` otherwise; then remove the vendor
      directories only if they are empty afterwards.
      **Why the drift check:** a copy that differs from the source is something
      someone edited in place. Deleting it silently discards work whose only
      copy was there.
- [ ] TASK-019: `src/core/harness/install/receipt.rs` — prune removed entries
      and rewrite the receipt after a successful run; delete the receipt itself
      only when it ends up empty or when `--purge` is given.
      **Why not always delete it:** a partial uninstall (one `--skill`) must
      leave a receipt describing what is still installed, or the next uninstall
      has nothing to work from.
- [ ] TASK-020: `src/cli/mod.rs` + `src/cli/commands/harness.rs` —
      `HarnessAction::Uninstall(HarnessUninstallArgs)` with `--scope`,
      `--vendor`, `--skill`, `--command`, `--all`, `--force`, `--purge`, run
      through `execute_and_report`. **`.ash/` is never a target of uninstall**,
      with or without `--purge`: the corpus is the repo's own record of work and
      outlives any projection of the skills that produced it. Say so in
      `--help`, not only in code.
- [ ] TASK-021: tests — uninstall after install leaves the tree byte-identical
      to before (including no empty `.claude/` left behind); a hand-written file
      at a managed path is reported `Blocked` and still exists afterwards; an
      entry absent from the receipt is never touched even when it sits in the
      managed directory and matches a source asset by name; `--dry-run` deletes
      nothing; `--purge` removes the receipt and a partial uninstall rewrites it.
      **Why the third one is the important test:** it is the whole safety
      property, and the only one that fails silently if the implementation ever
      grows a "clean up anything that looks like ours" shortcut.

## Trade-offs & risks
RISK-001 is concentrated in TASK-017 and constrained in TASK-018: the plan can
only ever contain paths read out of a receipt pinst wrote, every one of them is
visible under `--dry-run` like any other action, and drift blocks rather than
proceeds. `--force` overrides the drift check and nothing else — it does not
widen the set of paths considered.

`remove_dir_all` for a directory is the one genuinely sharp edge, because a
skill installed by copy *is* a directory. It is reached only for a path recorded
in the receipt as a copied skill, and only after the drift check has compared
its contents against the source.

Not deleting `.ash/` is a deliberate asymmetry with install, which creates it.
Creating an empty scaffold is cheap to undo by hand; deleting a corpus is not
undoable at all, and the two are not symmetric just because one command is
spelled as the other's opposite.

## Done criteria
- TEST-011: `cargo test` covers the round trip, the blocked-on-drift path, the
  not-in-receipt path, `--dry-run`, and `--purge`.
- TEST-012: in a tempdir, install then uninstall leaves `git status --short`
  empty relative to the pre-install tree.
- TEST-013: `pinst harness uninstall --scope project --all --purge` removes the
  receipt and leaves `.ash/` and every plan in it untouched.
- TEST-014: `pinst harness uninstall --json` reports `Blocked` steps in `items`
  with the reason, and exits `3` rather than `0`, so a caller can tell "removed
  everything" from "left things behind".
