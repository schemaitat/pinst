# pinst

One self-contained binary that installs, updates, and doctors this machine's
toolchain — and carries the dotfiles with it.

`pinst` replaces what used to be a `~/dotfiles` repo plus `bootstrap.sh`, a
`justfile`, and GNU Stow. A manifest is the single source of truth for the
tools; the configs are compiled into the binary, so provisioning a new machine
needs no clone, no `just`, and no `stow`.

```sh
pinst bootstrap --dry-run   # exactly what would happen, nothing touched
pinst bootstrap -y          # provision this machine
pinst doctor                # what is still off
pinst apply                 # converge an existing machine
pinst tui                   # interactive dashboard
```

Driving it from an agent or a script? Read [AGENTS.md](AGENTS.md) — every
command speaks `--json`, every mutating one speaks `--dry-run`, and the exit
codes are part of the contract.

## Install

On a fresh machine:

```sh
git clone <this repo> ~/.local/share/pinst/src
~/.local/share/pinst/src/scripts/install.sh
```

The script builds the release binary (installing Rust first if the machine has
none) and drops it in `~/.local/bin`. From there, `pinst bootstrap` does the
rest.

## How it is organized

| Path | What it is |
|------|------------|
| `manifest.toml` | Every tool: how to detect, install, and upgrade it, plus its dependencies and tags |
| `configs/<pkg>/` | The config tree, mirroring `$HOME`. Embedded into the binary at build time |
| `src/core/` | The engine: manifest, dependency graph, probing, planning, execution, configs, doctor |
| `src/cli/` | The command surface — thin, no logic |
| `src/ui/` | The TUI — also thin, over the same core |

Adding a tool is one `[[tool]]` entry in `manifest.toml`. Adding a new *install
method* is one module in `src/core/exec/` implementing the `Executor` trait.
Run `pinst schema manifest` for the authoritative field reference.

## Commands

| Command | |
|---------|--|
| `pinst list` | What the manifest declares, and what is actually installed |
| `pinst plan` | The ordered install plan (dependency-sorted), without doing anything |
| `pinst install [tools...]` | Install what is missing |
| `pinst update [tools...]` | Upgrade what has a newer version |
| `pinst config status\|diff\|apply\|adopt` | Inspect and apply the configs |
| `pinst doctor [--fix]` | Diagnose tools and configs; `--fix` repairs the safe subset |
| `pinst apply` | Converge everything, then diagnose |
| `pinst bootstrap` | `apply` for a fresh machine (`default` profile) |
| `pinst tui` | Dashboard: tools, findings, upgrades, `/` to search, `e` to edit a config |

Select a subset anywhere with positional names, `--profile <name>`, or
`--tag <tag>`. Naming a tool pulls in its dependencies automatically.

## Configs: linked or materialized

pinst picks based on whether the source tree is available:

- **Repo checked out** (or `$PINST_SOURCE` set) → `$HOME` is **symlinked** at
  `configs/`, so editing `~/.zshrc` edits the repo copy directly.
- **Binary only** (a freshly provisioned box) → files are **written** from the
  embedded copy.

`pinst config status` shows the state of each file: `linked`, `ok`, `missing`,
`drifted` (edited in place), or `foreign` (a symlink into some other tree).
`pinst config adopt` pulls a drifted file's contents back into the source tree;
`pinst config apply` goes the other way. An existing file is always backed up
to `<file>.pre-pinst.<timestamp>` — pinst never clobbers.

### Machine-specific values

Anything that differs per machine is a `${VAR}` placeholder resolved from
`~/.config/pinst/values.toml`, then the environment:

```toml
GIT_USER_NAME = "Ada Lovelace"
GIT_USER_EMAIL = "ada@example.com"
```

A missing value is a hard error, not an empty string written into your
`.gitconfig`. Secrets never live here: `.zshrc` sources `~/.zshrc.secrets` at
runtime, and that file is not tracked by pinst at all.

## Migrating off `~/dotfiles`

On a machine whose `$HOME` still symlinks into the old `~/dotfiles` repo,
`pinst doctor` reports each of those files as `config.foreign.*`. Cut over in
this order — **re-point first, delete last**, because your login shell is
reading those symlinks right now:

```sh
# 1. See what would change.
pinst config status
pinst config apply --dry-run

# 2. Re-point $HOME at pinst. Existing real files are backed up, and the old
#    repo is left untouched.
pinst config apply

# 3. Verify. Exit code 0 means nothing is outstanding.
pinst doctor; echo "exit: $?"

# 4. Only once doctor is clean, retire the old repo — archive, do not delete.
mv ~/dotfiles ~/dotfiles.retired.$(date +%Y%m%d)
```

If anything looks wrong at step 3, the old repo is still intact and its files
are unmodified; re-pointing a symlink back is a one-line fix.

From then on, config edits go through this repo: edit the file in `$HOME` (it
is linked to `configs/`), or edit `configs/` directly, and commit.

## Development

```sh
just            # list the recipes
just run        # run from source (opens the dashboard)
just run doctor --json   # ...or any other command; args forward
just qc         # formatting, lints, and tests — what CI would run
just build      # the self-contained release binary
```

`just run` propagates pinst's exit codes, so `just run doctor` ending in
"recipe failed with exit code 3" is pinst reporting findings, not a broken
recipe.

Plans and architectural decisions live in `.ash/plans/`.
