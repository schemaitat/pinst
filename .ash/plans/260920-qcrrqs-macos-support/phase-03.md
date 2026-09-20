---
id: 260920-qcrrqs
slug: macos-support
phase: 3
status: Proposed
---

# Phase 3 — A manifest and configs that cover macOS

## Goal

**GOAL-003**: give every remaining tool in the manifest a macOS answer —
an override, or an explicit `unsupported` note — and make the config packages
apply correctly on a Mac, so that `pinst plan --platform macos` describes a
machine someone would actually want.

## Why this phase exists

This is the phase where ASSUMPTION-003 stops being an assumption. Every
non-apt tool is claimed to work unmodified on macOS, and that claim is
per-tool: `nvm`, `uv`, `oh-my-posh`, `herdr`, `aven`, `opencode` and `claude`
each have their own installer with their own platform handling, and at least
one of them is likely to be wrong. Checking them is reading work that has to
happen once, tool by tool, and it does not belong in Phase 2 where it would
have been buried under the executor it has nothing to do with.

It is separated from Phase 4 because this phase is entirely manifest and
config data, verifiable on Linux through `--platform macos`, while Phase 4
touches CI and release machinery that this work must not be entangled with:
a macOS artifact that builds but installs a manifest with holes in it is worse
than no artifact, and this order is what prevents that.

## Steps

- [ ] TASK-017: `manifest.toml` (FILE-013) — audit each of the seven
      `curl_script` and `shell` entries (`oh-my-zsh`, `oh-my-posh`, `uv`,
      `nvm`, `rust`, `herdr`, `aven`, `opencode`, `claude`) against its
      installer's own documentation and record the verdict as a comment on
      the entry: portable as written, or needing an override.
      Why: LESSON-002 — a platform-side precondition should be checked
      against the real source while planning the change, not asserted and
      discovered during implementation. The comment is what stops the next
      reader from re-doing the audit, and what makes a wrong verdict
      attributable later.
- [ ] TASK-018: `manifest.toml` (FILE-013) — add `[tool.platform.macos]`
      blocks for the remaining apt tools that have a Homebrew formula:
      `curl`, `git`, `unzip`, `zsh`, `fd`, `gh`. Each sets
      `method = "brew"` and `requires = ["homebrew"]`.
      Why: `curl`, `git`, `unzip` and `zsh` ship with macOS, so their
      `detect` commands already succeed and the install step will normally be
      skipped — the override exists so that the one machine where it does not
      has an answer rather than a hole. `fd` additionally drops the
      `fdfind` post-install step, since Homebrew installs the binary under
      its real name (LESSON-004's neighbourhood: the Debian rename is a
      packaging detail, not a property of the tool).
- [ ] TASK-019: `manifest.toml` (FILE-013) — declare `build-essential`
      `unsupported` on macOS with the note
      `Xcode Command Line Tools provide cc/make: xcode-select --install`.
      Why: DEP-003. `xcode-select --install` opens a GUI dialog and cannot be
      driven non-interactively in the general case, so automating it would
      produce a step that hangs. An `unsupported` note that names the exact
      command is the honest form, and Phase 1's doctor finding is where a user
      will see it. Note that `tree-sitter-cli` requires `build-essential` and
      so will plan a `cargo install` whose compiler comes from the CLT rather
      than from a pinst-managed step — that is correct, and it is why the
      dropped-dependency behaviour in TASK-007 had to be "carry on", not
      "drop the dependents too".
- [ ] TASK-020: `manifest.toml` (FILE-013) — override `neovim` on macOS to
      `method = "brew", formulae = ["neovim"]`, replacing the
      `nvim-linux-x86_64.tar.gz` / `/opt` install.
      Why: the entry that proves `Platform::admits` was deliberately not the
      whole story. `github_release` is a portable *method* carrying a
      Linux-only *asset*, so nothing mechanical could have caught this — only
      the per-tool read in TASK-017. Brew also removes the `/opt` write and
      its `sudo`, and with it the `confirm = true` gate.
- [ ] TASK-021: `manifest.toml` (FILE-013) — override pinst's own entry on
      macOS so `asset` is `pinst-aarch64-apple-darwin.tar.gz`, and add an
      `[tool.platform.macos]` note that the Intel asset is selected by
      `install.sh` rather than here.
      Why: REQ-006. `pinst update pinst` is how a machine stays current, and
      an entry that fetches the musl tarball onto a Mac would install a binary
      that cannot execute. The manifest has one `asset` string and no
      architecture axis, so it names the architecture the overwhelming
      majority of Macs are; `install.sh`, which can read `uname -m`, is where
      the Intel case is handled (Phase 5, TASK-032). This asymmetry is
      recorded in the entry so it is not read as an oversight.
- [ ] TASK-022: `manifest.toml` (FILE-013) — on the `zsh` entry's `chsh`
      post-install step, extend `skip_if` so it also skips when the resolved
      zsh is absent from `/etc/shells`.
      Why: ASSUMPTION-002. macOS ships zsh as the default login shell, so the
      step is normally skipped by the existing check; but a *Homebrew* zsh
      lives outside `/etc/shells`, and `chsh -s` against a shell not listed
      there fails with a bare non-zero exit. Skipping is the right outcome —
      editing `/etc/shells` is a root action with a system-wide effect, well
      outside what a post-install step should do unasked.
- [ ] TASK-023: `configs/zsh/.zshrc` (FILE-014) — guard the
      `/opt/nvim-linux-x86_64/bin` `PATH` entry so it is added only when the
      directory exists, and add a `brew shellenv` eval guarded the same way,
      resolving the prefix from wherever `brew` actually is.
      Why: RISK-005 and CON-004. The Homebrew prefix differs between Apple
      Silicon (`/opt/homebrew`) and Intel (`/usr/local`), so the prefix must
      be discovered, never written down. Guarding both on existence keeps one
      `.zshrc` correct on both platforms, which matters because the config
      packages are not platform-aware — only tools are, and extending the
      override mechanism to configs to solve two `PATH` lines would be a
      schema change bought for nothing.
- [ ] TASK-024: `docs/tools/homebrew.toml` (FILE-020) and `README.md`
      (FILE-019) — add the docs page for the `homebrew` entry, and document
      the `brew` install method row, the `[tool.platform.<name>]` block with a
      worked example, and `--platform`.
      Why: `build.rs` declares `docs/tools` a build input, so a new manifest
      entry without a page ships a binary whose `pinst docs homebrew` falls
      back to capturing `--help` (LESSON-015 is why that rebuild is reliable
      at all). The README's install-method table is the reference the manifest
      comments point at; a method missing from it is a method nobody finds.

## Trade-offs & risks

- **ASSUMPTION-003** is resolved here, per tool, by TASK-017 — and the
  resolution may be "needs an override", in which case that override is
  written in TASK-018's neighbourhood. If an installer turns out to have *no*
  macOS path, the entry gets an `unsupported` note and the fallback order is
  the same as `build-essential`'s: report it, do not automate it.
- **ASSUMPTION-004** shows up in TASK-021 as a hard-coded `aarch64` asset.
  Should `macos-latest` and the Mac population both move, this line is the
  one to change; it is a single string, named here so it is findable.
- **RISK-001** bites hardest in this phase: every task is verified by reading
  a plan, and a plan that resolves cleanly proves only that the manifest is
  well-formed. `brew install neovim` succeeding is a Phase 5 criterion, not
  one this phase can claim.
- **RISK-005** is mitigated rather than removed. `.zshrc` is now correct on
  both platforms but is still a single file carrying both platforms'
  concerns, and the guards are the only thing keeping that readable. A third
  platform's worth of guards would be the signal that configs need the same
  override mechanism tools have.
- Accepted: `curl`, `git`, `unzip` and `zsh` get brew overrides that will
  almost never run, because macOS ships all four. The alternative — marking
  them unsupported because the system provides them — would report four
  info findings on every `pinst doctor` for tools that are present and
  working, which is worse than an install step that is correctly skipped.
- Accepted: `git-credential-manager` stays `manual` on both platforms. Its
  note is Linux-specific prose, and rewording it to cover both is a
  documentation change with no mechanism behind it; the `manual` method
  already does the right thing on a Mac.

## Done criteria

- **TEST-015**: every tool in the embedded manifest resolves under
  `Platform::MacOS` to either a supported effective tool or an `Unsupported`
  carrying a non-generated note — asserted as a test over the real embedded
  manifest, so a tool added later without a macOS answer fails `just qc`.
  Why a test and not a convention: an invariant that lives only in prose has
  already drifted (LESSON-008).
- **TEST-016**: `pinst plan --platform macos --json` from Linux orders
  `homebrew` before every brew step, contains no `apt` step and no `sudo`
  outside the Homebrew bootstrap, and lists `build-essential` as unsupported
  with the `xcode-select` note.
- **TEST-017**: the resolved macOS `neovim` entry is a brew install with no
  `confirm` gate and no `/opt` destination.
- **TEST-018**: the resolved macOS `pinst` entry names
  `pinst-aarch64-apple-darwin.tar.gz`, and the Linux one still names
  `pinst-x86_64-unknown-linux-musl.tar.gz`.
- **TEST-019**: `zsh`'s `chsh` step is skipped when the resolved zsh is
  absent from `/etc/shells` — exercised with a temporary file standing in for
  `/etc/shells` rather than against the real one (LESSON-018).
- **TEST-020**: `.zshrc` sourced under a shell where neither
  `/opt/nvim-linux-x86_64/bin` nor `brew` exists adds neither to `PATH` and
  exits zero.
- **TEST-021**: `just qc` is green, `pinst harness check` is clean, and
  `pinst docs homebrew` renders the authored page rather than a capture.
