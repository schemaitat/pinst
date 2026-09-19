# pinst

One self-contained binary that installs, updates, and doctors this machine's
toolchain — and carries the dotfiles with it.

A **manifest** declares every tool: how to detect it, how to install it, what
it depends on. The **configs** live in this repo and are compiled into the
binary, so provisioning a new machine needs no clone, no `just`, and no `stow`.

`pinst` replaces what used to be a `~/dotfiles` repo plus `bootstrap.sh`, a
`justfile`, and GNU Stow.

```sh
pinst bootstrap --dry-run   # exactly what would happen, nothing touched
pinst bootstrap -y          # provision a fresh machine
pinst doctor                # what is off right now
pinst apply                 # converge an existing machine
pinst tui                   # interactive dashboard
```

Driving it from an agent or a script? [AGENTS.md](AGENTS.md) is the machine
contract: every command speaks `--json`, every mutating one speaks
`--dry-run`, and the exit codes are part of the interface.

## Contents

- [Install](#install) · [Commands](#commands)
- [Managing tools](#managing-tools) — add, select, order, upgrade, remove
- [Managing configs](#managing-configs) — edit, add, drift, machine-specific values
- [What pinst will and will not do on its own](#what-pinst-will-and-will-not-do-on-its-own)
- [Migrating off `~/dotfiles`](#migrating-off-dotfiles) · [Development](#development)

## Install

On a fresh machine:

```sh
git clone <this repo> ~/.local/share/pinst/src
~/.local/share/pinst/src/scripts/install.sh
```

The script builds the release binary (installing Rust first if the machine has
none) and puts it in `~/.local/bin`. From there:

```sh
pinst bootstrap --dry-run    # read the plan first
pinst bootstrap -y           # then provision
```

## Commands

| Command | |
|---------|--|
| `pinst list` | What the manifest declares, and what is actually installed |
| `pinst plan` | The ordered install plan, without doing anything |
| `pinst install [tools...]` | Install what is missing |
| `pinst update [tools...]` | Upgrade what has a newer version |
| `pinst config status\|diff\|apply\|adopt` | Inspect and apply the configs |
| `pinst doctor [--fix]` | Diagnose tools and configs; `--fix` repairs the safe subset |
| `pinst apply` | Converge everything, then diagnose |
| `pinst bootstrap` | `apply` for a fresh machine (the `default` profile) |
| `pinst schema manifest\|output` | JSON Schemas, derived from the code |
| `pinst tui` | Dashboard: tools, findings, upgrades; `/` searches, `e` edits a config |

Global flags: `--json`, `--dry-run`, `--yes`/`-y`, `--quiet`/`-q`,
`--manifest <path>`.

---

# Managing tools

## Seeing what is there

```sh
pinst list                   # every tool, with installed state and version
pinst list --tag editor      # just one tag
pinst doctor                 # what is missing or wrong, with the fix for each
```

`pinst list` exits **3** when anything is missing, so `pinst list >/dev/null ||
echo "work to do"` is a valid check.

## Adding a tool

Append a `[[tool]]` entry to `manifest.toml`. Nothing else — no code change, no
rebuild to try it locally:

```toml
[[tool]]
name = "fzf"
summary = "Fuzzy finder"
tags = ["dev"]
requires = ["curl"]
detect = { command = "command -v fzf", bin = "fzf", version_cmd = "fzf --version" }
install = { method = "apt", packages = ["fzf"] }
```

Then check your work:

```sh
pinst plan fzf       # shows: install curl (skipped), apt update, install fzf
pinst install fzf    # do it
```

A malformed entry fails immediately and specifically, with exit code 2:

```
error: invalid manifest manifest.toml: tool 'broken' requires unknown tool 'ghost'
```

### The fields

| Field | | |
|-------|--|--|
| `name` | required | Unique; what you type on the command line |
| `detect` | required | How to tell if it is already installed |
| `install` | required | Method + its fields (below) |
| `summary` | optional | One line, shown by `list` |
| `tags` | optional | Used by profiles to select groups |
| `requires` | optional | Tools that must be installed first |
| `upgrade` | optional | Overrides the upgrade check derived from `install` |
| `post_install` | optional | Extra steps once it is installed |

`detect` takes `command` (run with `sh -c`; exit 0 means installed), plus
optional `bin` (the PATH binary name), `version_cmd`, and `version_regex`
(capture group 1 is the version; defaults to a generic semver pattern).

### Install methods

| Method | Required | Optional | Produces |
|--------|----------|----------|----------|
| `apt` | `packages` | | `sudo apt-get install -y <packages>` |
| `cargo` | `crate_name` | | `cargo install --locked <crate>` |
| `curl_script` | `url` | `shell`, `args`, `github_repo` | `curl -fsSL <url> \| <shell> -s -- <args>` |
| `shell` | `command` | | The command verbatim — the escape hatch |
| `github_release` | `repo`, `asset`, `dest` | `confirm` | Downloads the asset, extracts with `tar` |
| `nvm` | | `version` | Sources `nvm.sh` and installs that version |
| `git_clone` | `url`, `dest` | `depth` | `git clone` (upgrades with `git pull --ff-only`) |
| `manual` | `note` | | Nothing — doctor reports the note |

Examples of the less obvious ones:

```toml
# A piped installer. github_repo is optional but enables upgrade checking.
install = { method = "curl_script", url = "https://astral.sh/uv/install.sh", shell = "sh", github_repo = "astral-sh/uv" }

# When the documented invocation does not fit the piped-curl shape.
install = { method = "shell", command = "curl --proto '=https' -sSf https://sh.rustup.rs | sh -s -- -y" }

# A release tarball into /opt. confirm = true means it needs --yes.
install = { method = "github_release", repo = "neovim/neovim", asset = "nvim-linux-x86_64.tar.gz", dest = "/opt", confirm = true }

# No automated path; doctor surfaces the note instead of pretending.
install = { method = "manual", note = "No apt package. See https://example.com/install" }
```

`pinst schema manifest` is the authoritative field reference — it is derived
from the parser, so it cannot drift from what pinst actually accepts.

Adding a whole new install *method* is a module in `src/core/exec/`
implementing the `Executor` trait, plus a variant on `manifest::Install`.

### Dependencies and ordering

`requires` is what makes installs safe to run in any order — pinst
topologically sorts them, so `zsh` lands before `oh-my-zsh` before its plugin,
and `nvm` before `node`. Naming a tool pulls in its dependencies
automatically:

```sh
pinst install node     # also plans nvm and curl, in that order
```

A dependency cycle is rejected at load time, not discovered mid-install.

### Post-install steps

For work that is not "install the package" — making zsh the login shell,
symlinking Debian's `fdfind` to `fd`:

```toml
[[tool.post_install]]
description = "make zsh the default login shell"
command = "chsh -s \"$(command -v zsh)\""
skip_if = '[ "${SHELL##*/}" = zsh ]'    # already done → skipped
confirm = true                           # needs --yes
```

`skip_if` is evaluated immediately before the step runs, which is what keeps
these idempotent. A post-install step is skipped entirely if its tool failed
to install.

### Tags and profiles

Tags group tools; a profile selects tags:

```toml
[profile.minimal]
summary = "Shell only — enough to be comfortable on a throwaway box"
tags = ["base", "shell"]
```

```sh
pinst list --tag agents
pinst bootstrap --profile minimal
```

`bootstrap` uses the `default` profile when you do not name one.

Selection always includes dependencies, so `--tag agents` also lists `curl`
and `node` — anything the tagged tools need. A selection is a set of tools to
*make work*, not a literal filter.

### Upgrades

```sh
pinst update              # check everything, upgrade what is behind
pinst update ripgrep fd   # just these
```

The check method is derived from how the tool was installed — apt candidates,
crates.io, GitHub releases, the Node LTS index — or set explicitly:

```toml
upgrade = { method = "apt", package = "git-delta" }   # package name differs from the tool name
upgrade = { method = "none" }                          # opt out (rustup self-manages)
```

Results are cached for 24h in `~/.cache/pinst/upgrade_cache.json`. Failed
lookups are not cached, so one offline run does not blind the next day.

Tools installed by a piped script cannot be version-pinned or verified;
`doctor` reports those as `tool.unpinnable.*` rather than implying a guarantee
it cannot make.

### Removing a tool

Delete its `[[tool]]` entry. pinst then stops managing, checking, and
reporting it.

**It does not uninstall anything.** There is no `pinst uninstall`; removing
the entry only ends pinst's interest in the tool. Uninstall it yourself
(`sudo apt remove ...`, `cargo uninstall ...`) if you want it gone. Check
first that nothing else `requires` it — the manifest will refuse to load if
something does.

### Which manifest is in effect

First match wins:

1. `--manifest <path>`
2. `$PINST_MANIFEST`
3. `~/.config/pinst/manifest.toml`
4. `./manifest.toml` (so running from the repo picks up your edit immediately)
5. the copy compiled into the binary

`pinst list` prints which one it used on stderr. Worth knowing: a stray
`~/.config/pinst/manifest.toml` outranks the repo's, which is a confusing way
to find out your edits "did nothing".

**Editing `manifest.toml` is live for this machine, but the binary carries its
own copy for other machines.** Rebuild (`just build`) and reinstall to ship
manifest changes elsewhere.

---

# Managing configs

## How configs map to `$HOME`

Each directory under `configs/` is a package whose tree mirrors `$HOME`:

```
configs/zsh/.zshrc                      ->  ~/.zshrc
configs/nvim/.config/nvim/init.lua      ->  ~/.config/nvim/init.lua
configs/git/.gitconfig                  ->  ~/.gitconfig
```

Each package is declared in the manifest:

```toml
[[config]]
name = "git"
source = "git"              # the directory under configs/
summary = "gitconfig (git identity is templated)"
requires_tool = "git"       # documentation only — see below
templates = [".gitconfig"]  # files needing ${VAR} substitution
```

`requires_tool` is validated (it must name a real tool) but is **not enforced
yet** — configs are applied whether or not the tool is present. Treat it as
documentation of intent, not a gate.

## Linked or materialized

pinst picks based on whether the source tree is available:

- **Repo checked out** (or `$PINST_SOURCE` set) → `$HOME` is **symlinked** at
  `configs/`. Editing `~/.zshrc` edits the repo copy — there is no sync step.
- **Binary only**, on a provisioned box → files are **written** from the
  embedded copy.

Source resolution, first match wins: `$PINST_SOURCE`, then a `configs/`
directory beside a `manifest.toml` (in the working directory or above the
binary), then the embedded copy. `pinst config status` prints which one it
used.

## Editing a config

On a machine with the repo checked out, just edit the file:

```sh
nvim ~/.zshrc          # this *is* configs/zsh/.zshrc, via the symlink
cd ~/.local/share/pinst/src && git diff
```

On a machine without the repo, the file is a real copy. Edit it, then pull the
change back into the repo from a machine that has both (see *Drift*, below),
or edit the repo directly.

Either way: **rebuild and reinstall the binary to carry config changes to
other machines**, since the binary embeds its own copy.

## Adding a file to an existing package

Drop it in the right place under `configs/<package>/`, mirroring `$HOME`:

```sh
mkdir -p configs/nvim/.config/nvim/lua/plugins
cp ~/.config/nvim/lua/plugins/new.lua configs/nvim/.config/nvim/lua/plugins/
pinst config status      # the new file shows as missing (not yet linked)
pinst config apply       # link it
```

In link mode a new file is picked up immediately, with no rebuild.

## Adding a new package

```sh
mkdir -p configs/tmux
cp ~/.tmux.conf configs/tmux/.tmux.conf
```

```toml
[[config]]
name = "tmux"
source = "tmux"
```

```sh
pinst config apply
```

## Checking state

```sh
pinst config status    # every file
pinst config diff      # only the ones that are not already correct
```

| State | Means | What to do |
|-------|-------|------------|
| `Linked` | Symlink into the source tree | Nothing |
| `Materialized` | Real file, content matches | Nothing |
| `Missing` | Not there | `pinst config apply` |
| `Drifted` | Real file, edited in place, differs | Decide — see below |
| `Foreign` | Symlink into some *other* tree (usually a pre-cutover `~/dotfiles`) | `pinst config apply` |
| `Unrenderable` | A templated file whose `${VAR}` values are missing | Set them in `values.toml` |

`config diff` lists which files differ; it does not print a line-by-line diff.
For that, `diff configs/zsh/.zshrc ~/.zshrc`.

## Drift: which side wins?

A file that was edited in place and no longer matches the repo is `Drifted`.
Only you know which version is right, so pinst will not choose:

```sh
pinst config adopt     # keep the machine's edit → copy it into the repo
pinst config apply     # discard it → rewrite from the repo (backs up first)
```

This is why `doctor --fix` never touches drifted files — it would be guessing.
`adopt` refuses templated files (adopting a rendered `.gitconfig` would write
this machine's name and email over the placeholders) and reports them instead
of aborting.

## Machine-specific values

Anything that differs per machine is a `${VAR}` placeholder, resolved from
`~/.config/pinst/values.toml` (or `$PINST_VALUES`), then the environment:

```toml
# ~/.config/pinst/values.toml
GIT_USER_NAME = "Ada Lovelace"
GIT_USER_EMAIL = "ada@example.com"
```

Mark which files get substitution with `templates` on the config package. A
missing value is a hard error, never an empty string written into your
`.gitconfig`; the file is reported as `Unrenderable` and everything else still
applies.

Shell syntax is safe — only `${NAME}` placeholders pinst knows are touched, so
`$HOME` and `${SHELL##*/}` in `.zshrc` pass through untouched.

## Secrets

Secrets never go in `configs/` and are never embedded in the binary. The
pattern already in use: `.zshrc` sources `~/.zshrc.secrets` at runtime if it
exists, and that file is entirely outside pinst. A test fails if a secret-shaped
filename ever lands in the config tree.

---

## What pinst will and will not do on its own

Because it runs installers, this is worth being explicit about.

**Always:**

- Every mutation is planned first, so `--dry-run` shows exactly what a real
  run would do — it is the same plan, not a description of one.
- An existing file is backed up to `<file>.pre-pinst.<timestamp>`, never
  overwritten in place.
- A failed step never aborts the run; the rest continues and the failure is
  summarized.
- Re-running converges: everything already correct is skipped.

**Needs your say-so** — steps the manifest marks `confirm` (making zsh the
login shell, unpacking into `/opt`) do nothing without `--yes` or an
interactive confirmation. Off a TTY, and in `--json` mode, pinst never
prompts; it reports the step as needing authorization.

**Note:** `sudo` steps are not `confirm`-gated by default; `sudo` itself will
ask for your password. To see the blast radius first, `pinst plan` prints the
exact command every step would run, and `pinst plan --json` additionally
labels each one `user`, `sudo`, `remote_script`, or `network`.

**Never:** uninstalls a tool, picks a side on a drifted config, or prompts
when nothing can answer.

## Migrating off `~/dotfiles`

On a machine whose `$HOME` still symlinks into the old repo, `pinst doctor`
reports those files as `config.foreign.*`. Cut over in this order —
**re-point first, delete last** — because your login shell is reading those
symlinks right now:

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

If step 3 looks wrong, the old repo is intact and its files are unmodified;
pointing a symlink back is a one-line fix.

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

| Path | What it is |
|------|------------|
| `manifest.toml` | Every tool: detection, install, upgrade, dependencies, tags |
| `configs/<pkg>/` | The config tree, mirroring `$HOME`; embedded at build time |
| `src/core/` | The engine: manifest, graph, probing, planning, execution, configs, doctor |
| `src/cli/` | The command surface — thin, no logic |
| `src/ui/` | The TUI — also thin, over the same core |

Plans and architectural decisions live in `.ash/plans/`.
