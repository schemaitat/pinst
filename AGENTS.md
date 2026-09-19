# pinst — the agent contract

`pinst` manages this machine's toolchain and configs from a manifest. It is
built to be driven by an automated agent: every command is non-interactive by
default, speaks JSON, and reports what it would do before doing it.

This file is the contract. `pinst schema manifest` and `pinst schema output`
emit the machine-readable versions of it.

## The rules that matter

1. **`--json` on every command.** The payload goes to stdout; all human
   progress goes to stderr. `pinst <cmd> --json` is always safe to pipe.
2. **`--dry-run` on every mutating command.** It emits the exact plan that a
   real run would execute — the same step list, in the same order. Run it
   first.
3. **Never prompt.** In `--json` mode, or when stdout is not a terminal, pinst
   never asks a question. Steps that need authorization are reported as
   `needs_confirmation` instead of blocking. Pass `--yes` to authorize them.
4. **Idempotent.** Re-running converges; a machine already in the desired
   state plans zero work and reports every step as `skipped`.
5. **Exit codes carry the verdict.** Branch on them; do not parse prose.

## Exit codes

| Code | Meaning | Typical response |
|------|---------|------------------|
| `0` | Success, nothing outstanding | done |
| `1` | A step failed, or the command could not complete | read `errors[]` |
| `2` | Usage error (bad flag, unknown tool/profile) | fix the invocation |
| `3` | Ran fine, but found things to act on (missing tools, config drift, blocked steps) | read `items[]` and act |

## The output envelope

Every command emits this shape (`pinst schema output` for the formal schema):

```json
{
  "schema_version": 1,
  "command": "install",
  "status": "ok | issues | error",
  "dry_run": false,
  "items": [ /* command-specific */ ],
  "errors": ["step-id: what went wrong"],
  "summary": { /* command-specific counts */ }
}
```

## Commands

| Command | What it does | `items[]` |
|---------|--------------|-----------|
| `pinst list` | Manifest inventory with live install status | tool status objects |
| `pinst plan` | The ordered install plan; never executes | plan steps |
| `pinst install [tools...]` | Installs missing tools | step reports |
| `pinst update [tools...]` | Upgrades tools with a newer version available | step reports |
| `pinst config status\|diff\|apply\|adopt` | Inspects and applies the configs | file statuses / step reports |
| `pinst doctor [--fix]` | Diagnoses tools + configs | findings |
| `pinst apply` | Converges everything: install, configs, then diagnose | step reports |
| `pinst bootstrap` | `apply` with the `default` profile, for a fresh machine | step reports |
| `pinst schema manifest\|output` | JSON Schemas | (raw schema on stdout) |
| `pinst tui` | Interactive dashboard (refuses `--json`) | — |

### Selecting tools

`list`, `plan`, `install`, `update`, `apply`, and `bootstrap` all take the same
selection arguments:

- positional names: `pinst install ripgrep fd`
- `--profile <name>`: every tool carrying that profile's tags
- `--tag <tag>` (repeatable): every tool carrying that tag

Naming a tool always pulls in its `requires` closure, so `pinst install node`
also plans `nvm` and `curl`, in dependency order. With no selection, the whole
manifest is used.

## Step outcomes

`install`/`update`/`apply`/`bootstrap` items each carry an `outcome`:

| Outcome | Meaning |
|---------|---------|
| `ran` | Executed successfully |
| `would_run` | `--dry-run` only; this is what would have happened |
| `skipped` | Already satisfied — the idempotency signal |
| `needs_confirmation` | Privileged/risky; re-run with `--yes` |
| `blocked` | No automated path exists; `detail` holds the instructions |
| `failed` | Ran and failed; `detail` holds the error |

A failed step never aborts the rest of the run.

## Doctor findings

`doctor` items carry a **stable `id`** safe to match on:

| id pattern | Severity | Fixable |
|------------|----------|---------|
| `tool.missing.<name>` | error | yes |
| `tool.manual.<name>` | warning | no — has no automated install |
| `tool.unpinnable.<name>` | info | no — installed from an unpinned remote script |
| `config.missing.<path>` | error | yes |
| `config.foreign.<path>` | warning | yes — points into another tree |
| `config.drifted.<path>` | warning | **no — a human must choose** adopt vs. discard |
| `config.template.<path>` | error | no — set the missing values |
| `shell.not-zsh` | warning | no |

Every finding carries a `remediation` string: the literal command that fixes
it. `doctor --fix` applies only the `fixable` subset.

## Extending the manifest

The manifest (`manifest.toml`, or `$PINST_MANIFEST` / `~/.config/pinst/manifest.toml`)
is the source of truth. To add a tool, append a `[[tool]]` entry:

```toml
[[tool]]
name = "fzf"
summary = "Fuzzy finder"
tags = ["dev"]
requires = ["curl"]          # enforced install ordering
detect = { command = "command -v fzf", bin = "fzf", version_cmd = "fzf --version" }
install = { method = "apt", packages = ["fzf"] }
```

Install methods: `apt`, `cargo`, `curl_script`, `shell`, `github_release`,
`nvm`, `git_clone`, `manual`. Run `pinst schema manifest` for every field of
every method — it is derived from the parser, so it cannot drift from what
pinst actually accepts.

Validation runs on every load and fails loudly on duplicate names, unknown
`requires` targets, dependency cycles, and profiles selecting tags no tool
carries. Check your edit with:

```sh
pinst list --json >/dev/null    # exits 2 with the specific problem if invalid
```

## Configs

Configs live in `configs/<package>/`, mirroring `$HOME`. They are compiled
into the binary, so a downloaded pinst can provision a machine with no clone
and no network.

- **Source tree present** (this repo checked out, or `$PINST_SOURCE` set):
  `$HOME` is **symlinked** at the tree, so edits round-trip immediately.
- **Binary only**: files are **written** from the embedded copy.

Machine-specific values use `${VAR}` and resolve from
`~/.config/pinst/values.toml` (or `$PINST_VALUES`), then the environment. An
unresolved variable is a hard error, never a silently empty config:

```toml
# ~/.config/pinst/values.toml
GIT_USER_NAME = "Ada Lovelace"
GIT_USER_EMAIL = "ada@example.com"
```

Secrets are never embedded. `.zshrc` sources `~/.zshrc.secrets` at runtime if
it exists; that file stays untracked and outside pinst entirely.

## Safety model

- Mutations only ever happen through a plan, so `--dry-run` and a real run
  cannot diverge.
- Each step declares a `privilege`: `user`, `sudo`, `remote_script`
  (a piped installer, unverifiable by construction), or `network`.
- Steps flagged `confirm` (making zsh the login shell, unpacking into `/opt`)
  require `--yes`.
- An existing config file is **backed up** to `<file>.pre-pinst.<timestamp>`,
  never clobbered.

## Typical agent flow

```sh
pinst doctor --json; [ $? -eq 3 ] && echo "work to do"
pinst apply --dry-run --json      # inspect the plan
pinst apply --yes --json          # converge
pinst doctor --json               # verify: exit 0 means clean
```

## Working *on* pinst

Everything above is about driving pinst. If you are changing pinst itself,
[`.agents/README.md`](.agents/README.md) is the contract for that: the
plan → implement → learn lifecycle, the skills in `.agents/skills/` that
carry it, the `.ash/` corpus of past decisions, and the checks that keep
them consistent.

```sh
just harness      # skills wired, plan corpus consistent — 0 clean, 3 findings
just qc           # fmt, clippy, tests, and the above
```
