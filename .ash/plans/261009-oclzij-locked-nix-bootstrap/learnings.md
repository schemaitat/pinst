---
id: 261009-oclzij
slug: locked-nix-bootstrap
updated: 2026-10-09
areas: [bootstrap, exec, nix, configs]
issue_count: 9
---

# Learnings — Locked Nix bootstrap

## Source
- Plan: `./README.md`
- Basis: session context, cross-checked against the implementation log
- Logs consulted: `logs/20261009T081457Z-gpt-6-astra.log`

## Summary
All four phases were implemented locally: legacy failure handling, locked Nix
packages, explicit Home Manager migration/rollback, and offline shell/editor
readiness. `just qc` passed 348 Rust tests and five installer tests; the complete
Nix-built lifecycle passed with Docker networking disabled. All four deployment
derivations evaluate; native macOS execution remains a CI result to obtain after
publication. The live account and its uncommitted dotfiles were not activated.

## Issues

### ISSUE-001: Estimated log timestamps ran ahead of the clock
**What happened:** Early phase-1 progress timestamps were ahead of an actual clock read.
**Root cause:** Timestamps were estimated instead of sampled at append time.
**Fix applied:** Appended a correction without rewriting history; subsequent checkpoints use actual UTC output.
**Recommendation:** Sample the clock rather than inferring elapsed time.
**Skill:** none
**Distilled:** declined — the logging contract already requires actual UTC events; this was an execution mistake.
**Gap:** answered — timestamp collection belongs to the existing implementation workflow.

### ISSUE-002: Package availability differed across channels and systems
**What happened:** Unstable built on Linux but rejected Intel macOS. Stable 26.05 retained Intel support but did not package Herdr.
**Root cause:** Availability on unstable did not imply availability on a compatible release; shallow flake inspection did not force dependencies.
**Fix applied:** Locked stable 26.05, declared Herdr external, and evaluated actual deployment `.drvPath` attributes on all four systems.
**Recommendation:** Verify the exact channel and full derivations before relying on a package inventory. This recurs under LESSON-002.
**Skill:** none
**Gap:** answered — package evaluation is domain verification, not a new skill.

### ISSUE-003: Worktree and image metadata were not portable into Docker
**What happened:** Docker exec hit an image `/etc/group` symlink issue; the mounted checkout's `.git` pointed into absent host worktree metadata.
**Root cause:** The container could not reuse host Git administrative paths, and the image/runtime combination could not resolve the group file during exec.
**Fix applied:** Use one-shot containers and a shared Nix store volume. Export tracked working-tree bytes into a clean source directory.
**Recommendation:** Test a portable source snapshot and keep build caches separate from the fresh test home.
**Skill:** none
**Distilled:** declined — the repository-specific container runner now encodes this topology.
**Gap:** answered — fixture topology belongs to the e2e runner.

### ISSUE-004: Tests assumed tools lived under /usr/bin
**What happened:** The Nix package compiled but failed two tests using only fake binaries and `/usr/bin:/bin` as PATH.
**Root cause:** Fixture utilities such as cut, grep and cp are store paths inside Nix's sandbox.
**Fix applied:** Keep fake binaries first, followed by the inherited build PATH. The Nix package's full suite then passed without disabling tests.
**Recommendation:** Shadow the intended executable while retaining declared runtime utilities. This reinforces LESSON-029.
**Skill:** none
**Gap:** answered — test dependency isolation is ordinary implementation work.

### ISSUE-005: Copilot's packaged tag archive failed its checksum
**What happened:** Nix rejected the stable copilot.lua v2.0.4 archive because its content differed from the fixed hash.
**Root cause:** Fetched bytes and the pinned derivation disagreed; this run did not establish why upstream differed.
**Fix applied:** Instead of accepting a replacement hash for that tag, used the immutable commit already in this repo's lazy-lock.json, inspected its loader, and computed its fixed source hash. Packaged the server separately and configured its explicit path.
**Recommendation:** Preserve source identity when verification fails; a new checksum is a source decision, not a retry. Promoted as LESSON-044.
**Skill:** none
**Gap:** answered — source identity checks belong to dependency maintenance.

### ISSUE-006: Built parsers were absent from Neovim's runtime path
**What happened:** The generation built, but an offline smoke test could not load the Bash parser, even after loading nvim-treesitter.
**Root cause:** Lazy takes over before Neovim's normal package loading, so Home Manager's parser dependencies were not automatically on runtimepath.
**Fix applied:** Registered the parser/query dependency closure explicitly before Lazy setup; verified every configured parser, native fzf, and Lua LS/Pyright initialization without networking.
**Recommendation:** Verify configured workloads after activation, not only package presence. Promoted as LESSON-045.
**Skill:** none
**Gap:** answered — application readiness belongs to bootstrap verification.

### ISSUE-007: Existing editor fixture had a transient executable-busy failure
**What happened:** One quality run failed the existing editor test with ETXTBSY.
**Root cause:** Not established; the unchanged fixture passed in isolation and subsequent full runs.
**Fix applied:** Ran the isolated test and a bounded full-gate retry. No test or product behavior was weakened.
**Recommendation:** Preserve diagnostic evidence and isolate transient failures before changing product code.
**Skill:** none
**Distilled:** declined — no reusable root cause was established.
**Gap:** answered — fixture diagnostics are covered by existing verification.

### ISSUE-008: The Git smoke test selected only one global file
**What happened:** Preserved identity was in the runtime include, but `git config --global user.name` did not see it.
**Root cause:** Explicit global-file selection did not reproduce normal Git operations' effective lookup across XDG config.
**Fix applied:** Verify `git config user.name` inside the new login shell; the full lifecycle passed with the expected identity.
**Recommendation:** Test effective application behavior rather than an inspection mode that changes the lookup.
**Skill:** none
**Distilled:** declined — this Git scope distinction is recorded here and exercised by e2e.
**Gap:** answered — application smoke checks belong to the existing suite.

### ISSUE-009: A lexical Nix binding shadowed a parser attribute
**What happened:** `with p; [ lua ... ]` resolved lua to a generated-config derivation and failed on a missing grammar version.
**Root cause:** Lexical bindings take precedence over names introduced by with.
**Fix applied:** Renamed the generated configuration binding to configLua.
**Recommendation:** Use unambiguous local names or explicit p.lua-style selections.
**Skill:** none
**Distilled:** declined — a localized namespace collision caught before activation.
**Gap:** answered — language diagnostics do not need a separate workflow skill.
