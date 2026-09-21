---
id: 260920-qcrrqs
slug: macos-support
phase: 1
status: Done
---

# Phase 1 — Platform becomes a value pinst can reason about

## Goal

**GOAL-001**: give pinst a `Platform` it carries as a parameter, an
`[tool.platform.<name>]` override block in the manifest, and a
`tool.unsupported.<name>` doctor finding — so that a Linux machine can render
and inspect the macOS answer with `--platform macos` before any macOS-specific
install path exists.

## Why this phase exists

Every later phase is unverifiable without this one. The work is being done on
Linux, and `--platform macos` is the only way to see a macOS plan without a
Mac (RISK-001); shipping the brew executor first would mean writing it blind
and testing it never. It is also the phase that decides platform is a
*parameter* rather than a `cfg!` read (GUD-001, ALT-003) — a choice that gets
harder to reverse with every call site added, so it belongs before the call
sites exist.

It deliberately does not introduce `Install::Brew`. This phase's override
mechanism must be provable on overrides that use only the methods that already
exist, otherwise a bug in resolution and a bug in the new executor arrive in
the same commit with no way to tell them apart.

## Steps

- [x] TASK-001: `src/core/platform.rs` (FILE-001) — add
      `enum Platform { Linux, MacOS }` with `Display`/`FromStr` over `linux`
      and `macos`, `Platform::host()` reading `cfg!(target_os)`, and
      `Platform::resolve(explicit: Option<Platform>)` preferring the explicit
      value, then `PINST_PLATFORM`, then the host.
      Why: three sources with a fixed precedence, decided once, so no call
      site ever re-derives it. An unknown `PINST_PLATFORM` value is a usage
      error (exit 2) rather than a silent fall back to the host — a typo that
      quietly plans for the wrong platform is the failure this whole flag
      exists to avoid.
- [x] TASK-002: `src/core/platform.rs` (FILE-001) — add
      `Platform::admits(&Install) -> bool`, returning false for
      `Install::Apt` on `MacOS` and true for every other method on every
      platform.
      Why: this is the table that makes an uncovered tool detectable without
      a new manifest field. `apt` is the only method today that names its
      platform; the rest (`cargo`, `curl_script`, `shell`, `github_release`,
      `nvm`, `git_clone`, `manual`) are portable by construction, and the ones
      that are portable-by-method but not portable-in-fact — `neovim`'s Linux
      asset — are handled by an explicit override in Phase 3, not by this
      table.
- [x] TASK-003: `src/core/mod.rs` (FILE-002) — register `pub mod platform;`.
- [x] TASK-004: `src/core/manifest.rs` (FILE-003) — add
      `struct PlatformOverride` with optional `detect`, `install`, `upgrade`,
      `requires`, `post_install` and `unsupported: Option<String>`, and a
      `#[serde(default)] platform: BTreeMap<String, PlatformOverride>` field
      on `Tool`. Deny unknown fields on the new struct.
      Why: `deny_unknown_fields` per LESSON-017 — this is a format a human
      hand-writes, and a mistyped `instal` that silently resolves to the Linux
      default is indistinguishable from a tool that simply has no override.
- [x] TASK-005: `src/core/manifest.rs` (FILE-003) — add
      `Tool::resolve(&self, platform: Platform) -> Resolution`, returning
      either the effective `Tool` (base fields with the override's `Some`
      fields substituted) or `Unsupported(note)`. A tool is unsupported when
      its override says so explicitly, or when no override exists and
      `platform.admits(&self.install)` is false; the note in the second case
      is generated (`"<name> installs with apt, which macOS does not have"`).
      Why: the generated note is what keeps a half-covered manifest honest
      without demanding a hand-written sentence for all fourteen apt tools
      before anything can be tried.
- [x] TASK-006: `src/core/manifest.rs` (FILE-003) — extend `validate()` to
      reject an override keyed by a platform name that does not parse, and to
      reject an override that sets both `unsupported` and any substituting
      field.
      Why: `[tool.platform.darwin]` is the spelling a reader will reach for
      first and it would otherwise be accepted and ignored — the exact silent
      failure LESSON-008 describes. Validation runs on every load, so this is
      caught at `pinst list`, not at install time.
- [x] TASK-007: `src/core/graph.rs` (FILE-004) and `src/cli/mod.rs`
      (FILE-006) — thread `Platform` into `select`: resolve each tool, drop
      the unsupported ones before topological ordering, and return them
      alongside the selection so callers can report them. Add the global
      `--platform <linux|macos>` flag to `GlobalArgs` and carry the resolved
      value on `Ctx`.
      Why: filtering before `topo_order` rather than after is what stops an
      unsupported tool's `requires` edges from ordering a plan around
      something that will never be installed. A tool whose *dependency* was
      dropped stays selected and will fail honestly at install time — pinst
      reports per-tool outcomes and one failure must not abort the rest, which
      is `engine.rs`'s existing contract.
- [x] TASK-008: `src/core/doctor.rs` (FILE-005) — emit
      `tool.unsupported.<name>` at `Severity::Info`, `fixable: false`, with
      the resolution's note as `remediation`, for each tool dropped by
      TASK-007.
      Why: info, not warning — an apt tool on a Mac is a correct absence, and
      a check that fires on a normal state is a check that gets switched off
      (LESSON-011). It is reported at all so `pinst doctor` on a Mac accounts
      for the whole manifest rather than quietly listing fewer tools than the
      manifest contains.

## Trade-offs & risks

- **RISK-001** is created here and never fully retired: `--platform macos` on
  Linux exercises parsing, resolution, filtering, ordering and rendering, and
  cannot exercise a single install. Everything this phase can prove, it proves
  from Linux; everything it cannot is deferred to Phase 5's second list.
- **ASSUMPTION-003** gets its mechanism here but not its answer. `admits()`
  returns true for `github_release`, so `neovim` will resolve as *supported*
  on macOS while still pointing at `nvim-linux-x86_64.tar.gz`. That is
  deliberate — the table describes methods, and only a human reading the
  entry knows the asset is Linux-specific. Phase 3 is where each such tool is
  checked individually; until then the macOS plan is knowingly wrong for that
  one tool, which is safe because nothing runs it.
- Accepted: `Platform` is a closed two-variant enum. A third platform is a
  variant plus an `admits` row, but it is not free, and pretending otherwise
  with a string-typed platform would cost the compile-time exhaustiveness
  that makes the `admits` table trustworthy.
- Accepted: `--platform` is a global flag on every subcommand, including ones
  where it means nothing (`harness`, `schema`). Restricting it to the
  commands that consult the manifest would mean three flag definitions to
  keep in step; a globally accepted flag that some commands ignore is the
  cheaper inconsistency.
- Deferred: `StepKind::BrewUpdate` and the `Install::Brew` variant. They
  belong with the executor that emits them (LESSON-016), which is Phase 2.

## Done criteria

- **TEST-001**: `Platform::resolve` unit tests covering all three sources and
  their precedence, plus an unrecognized `PINST_PLATFORM` producing a
  `UsageError`. The env var is read through the resolve parameter rather than
  `std::env` at the call site, so the tests set no ambient state
  (LESSON-018).
- **TEST-002**: `Tool::resolve` unit tests over an inline manifest fixture:
  an override substituting `install` only, leaving `detect` inherited; an
  explicit `unsupported`; an apt tool with no override resolving to
  `Unsupported` with the generated note; and a non-apt tool with no override
  resolving unchanged on both platforms.
- **TEST-003**: `validate()` rejects `[tool.platform.darwin]` and rejects an
  override carrying both `unsupported` and `install`, each with an error
  message naming the tool.
- **TEST-004**: with a `[tool.platform.macos]` override added to exactly one
  existing manifest entry — `fd`, whose `fdfind` symlink post-install step is
  Debian-specific — `pinst plan --platform macos --json` from Linux omits the
  fourteen apt tools, includes `fd` with an empty `post_install`, and
  `pinst doctor --platform macos --json` reports fourteen
  `tool.unsupported.*` findings at info severity.
  Why this tool: it is the one entry whose *post-install* differs rather than
  its install, so it proves per-field substitution rather than whole-`install`
  replacement.
- **TEST-005**: `pinst plan --json` with no `--platform` on Linux produces
  byte-identical output to the same command before this phase. Platform
  resolution defaulting to the host must be a no-op for every existing caller
  (CON-005).
- **TEST-006**: `just qc` is green — including `cargo fmt --check` and
  `clippy -D warnings`, and the embedded manifest still validating.
