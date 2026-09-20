---
id: 260920-tensvp
slug: tool-docs-explorer
phase: 3
status: Proposed
---

# Phase 3 — Capture what is not written yet, and adopt it

## Goal
**GOAL-003**: make an unwritten page still answer something useful — capture
the tool's own help from the machine, cache it against the installed version,
and offer to write it back into the repo as a draft page (REQ-005).

## Why this phase exists
Phase 2 ends with 25 of 26 tools answering "no page". Capture is what turns
that from a hole into a degraded answer, and adopt is what makes authoring
phase 5 a matter of editing seeded files rather than typing 26 from nothing.
It has to come after the read path because the fallback is a branch *inside*
`show` — writing it first would mean building the subprocess machinery with
no consumer to shape it — and before search, because a captured page is
content search will index.

## Steps
- [ ] TASK-015: `src/core/manifest.rs` — optional `help_cmd: Option<String>`
      on `Tool`, run with `sh -c` like `detect.command`. Default when absent:
      `<detect.bin> --help`; no `bin` and no `help_cmd` means capture is not
      possible for that tool, which is a reportable state, not an error.
      Why on `Tool` rather than inside `detect`: `detect` answers "is it here
      and at what version", and help is a different question about the same
      binary.
- [ ] TASK-016: `src/core/docs/capture.rs` (new) — run the help command with a
      timeout (tokio, already a dependency), a hard cap on captured bytes, and
      `NO_COLOR=1`/`TERM=dumb` in the child environment; truncate rather than
      fail when the cap is hit, recording that it was truncated.
      Why the env forcing: a tool that detects a pipe and emits ANSI anyway
      would put escape sequences into a page and into an agent's context.
- [ ] TASK-017: `src/core/docs/capture.rs` — cache to
      `${XDG_CACHE_HOME:-$HOME/.cache}/pinst/docs/<tool>.txt` with the probed
      version recorded alongside, and treat a version mismatch as a miss.
      Why version-keyed: it is the one cache key that invalidates exactly when
      the answer changes, and pinst already probes the version for every tool.
- [ ] TASK-018: `src/cli/commands/docs.rs` — wire the fallback into `show`:
      a known tool with no page captures instead (fresh capture behind
      `--refresh`), and the item reports `source: "page" | "captured"` so a
      consumer can tell authored prose from a dump of `--help`. A tool with
      no page and no capture path keeps the phase-2 exit 3 and its remediation.
      Why a distinct `source` field rather than faking a page: the two have
      very different reliability, and collapsing them would hide that (RISK-003).
- [ ] TASK-019: `src/cli/commands/docs.rs` — `adopt <tool>`: write the capture
      into `docs/tools/<tool>.toml` as `status = "draft"` with the raw text in
      `help` and `what` seeded from the manifest `summary`. Refuse when the
      source is `Embedded` (there is no tree to write into) and refuse to
      overwrite a page whose `status = "authored"` — both exit 3 with the
      remediation. Honour `--dry-run` by printing the file it would write, and
      require `--yes` to overwrite an existing draft.
      Why it mirrors `config adopt`: the machine-seeds-the-repo round-trip
      already exists here, and reusing its rules means one set of expectations.
- [ ] TASK-020: `manifest.toml` — `help_cmd` for the tools whose help is not
      `<bin> --help`, and nothing for the ones that have no CLI at all
      (`oh-my-zsh`, `zsh-autosuggestions`, `build-essential`), which are
      page-only by nature.
      Why: each of these was a guess until run; the task is to run the command
      for all 26 and record what actually answers (LESSON-001).
- [ ] TASK-021: tests — a fake executable in a `tempfile` dir exercising:
      successful capture, cache hit without re-running, version change busting
      the cache, a command that exceeds the timeout, and output past the cap
      being truncated rather than erroring. `adopt` tested against a temp tree
      for the write, the embedded refusal, and the authored-page refusal.

## Trade-offs & risks
- **SEC-001** lands in this phase. The constraints that make it acceptable —
  manifest-declared tools only, never a name from a query, never during
  `dump`, timeout, byte cap, neutered child env — are all TASK-016/018, and
  the trust boundary is unchanged because the manifest already reaches `sh -c`
  through `detect.command` and `post_install.command`.
- A captured page is machine-specific and version-specific. It stays in the
  cache, never in git (ALT-010); only adopt's draft, which a human then
  edits, crosses into the repo.
- The cache is written under `$HOME` by a read-only-looking command. Accepted
  because it is a cache directory by convention, it is keyed so it cannot go
  stale, and the alternative — re-running every tool's help on every lookup —
  makes `show` unpredictably slow.

## Done criteria
- **TEST-006**: the five capture cases above are covered, including the
  timeout and the truncation, with no test depending on a real tool being
  installed.
- **TEST-007**: `adopt` writes a parseable page that TEST-001's validation
  accepts, refuses without a source tree, and refuses to clobber an authored
  page — each asserted with its exit code.
- `pinst docs show delta` returns captured help on this machine and reports
  `source: "captured"`; a second run does not re-execute the tool.
- `just qc` is green.
