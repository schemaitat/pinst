# Locked bootstrap

The Nix path builds a known package closure before activating a Home Manager
generation. `flake.lock` pins Nixpkgs and Home Manager. The package set uses
26.05 because it supports Intel macOS as well as Linux and Apple Silicon.
The existing manifest-driven bootstrap remains available on unmanaged homes.

## Build before activating

From a checkout at the revision you want to deploy:

```sh
sh scripts/bootstrap-nix.sh --dry-run
sh scripts/bootstrap-nix.sh --yes
if [ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]; then
  . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
fi
export PATH="$HOME/.local/state/pinst/environment/pinst/bin:$PATH"
pinst environment build --flake . --configuration andre-linux --dry-run --json
pinst environment build --flake . --configuration andre-linux --json
pinst environment apply --dry-run --yes --json
pinst environment apply --yes --json
pinst environment status --json
```

`bootstrap-nix.sh` downloads the pinned NixOS installer and verifies its
committed SHA-256 before execution. Intel macOS uses the versioned upstream
shell installer, also verified against a committed SHA-256; that installer
verifies its platform tarball. Linux/WSL installation expects systemd and
available sudo credentials. Existing Nix installations are reused.

It builds the package-only toolchain and pinst itself, without activating home
configuration. The CLI is retained at
`~/.local/state/pinst/environment/pinst/bin/pinst`. After activation, open a
new Zsh login shell (`exec "$HOME/.nix-profile/bin/zsh" -l`) to load the
managed PATH. Changing the account's default login shell is a host-level task.

Build without `--configuration` for the package-only pilot. Its binaries are
under `~/.local/state/pinst/environment/build/bin`; add that directory to a
temporary shell's PATH to try them. This does not change dotfile ownership.

The named configurations live in `nix/hosts.nix`. Add your username, absolute
home directory and system there; stage new files before building from a Git
checkout. `andre-linux`, `andre-linux-arm64`, `andre-macos` and
`andre-macos-intel` cover the corresponding platforms. `test-*` configurations
are disposable test accounts, never deployment defaults.

The default build result is a GC-root symlink at
`~/.local/state/pinst/environment/build`. Override it with `--out-link PATH`.
Activation accepts `--generation PATH`, resolves that link once, checks the
generation's declared home against `$HOME`, and activates that exact store
path. A build alone never activates anything. Dry-run only reads local inputs
and prints the plan; it does not fetch, build, update locks, or write files.

## Migrate existing dotfiles

First compare the live dotfiles with this checkout. In particular, this
machine's `~/dotfiles` has uncommitted Zsh and Herdr edits. The migration uses
the new generation's configuration; it does not silently import those edits
into Git or re-source the entire old shell configuration.

```sh
git -C ~/dotfiles diff
pinst environment apply --migrate --dry-run --yes --json
pinst environment apply --migrate --yes --json
```

Without `--migrate`, conflicting files block activation. With it, pinst writes
a receipt under `~/.local/state/pinst/environment/migrations/<id>/` and then:

1. Copies each conflicting file/directory's live contents to `snapshot/`.
2. Moves the original file or symlink to the matching path under `original/`.
3. Runs the built Home Manager activation script.

The old dotfiles repository is never changed. Snapshots preserve uncommitted
edits even if its source changes later. Original relative symlinks may appear
dangling inside the backup; their original meaning returns when moved back
to the same location under `$HOME`. A symlinked broad parent such as
`~/.config` is rejected rather than moving unrelated configuration.

Only `user.name` and `user.email` are carried from an existing `.gitconfig`
into `~/.config/pinst/git-identity`, if that include does not already exist.
Inspect the snapshots to port other local settings deliberately. Shell-local
additions can live in `~/.config/pinst/local.zsh`. Secrets stay in
`~/.zshrc.secrets` and are loaded at runtime. Never place secrets in tracked
flake inputs: those inputs are copied into the Nix store.

## Rollback and recovery

After a successful activation of a different generation:

```sh
pinst environment rollback --dry-run --yes --json
pinst environment rollback --yes --json
pinst environment status --json
```

`current`, `previous`, and `candidate` under the environment state directory
retain store paths as GC roots. A failed activation does not replace pinst's
last successful `current` record. Home Manager activation is not a transaction
over arbitrary application state: inspect failures and use the retained
generation to recover. The first activation has no previous Nix generation.

Generation rollback is distinct from undoing the initial migration. To
restore an original file, inspect `receipt.json`, move the new target aside,
and move its matching `original/<relative-path>` back to `$HOME/<relative-path>`.
Use `snapshot/` for recovery if the old source repo no longer exists. Preserve
the migration directory until recovery is verified. Restoring dotfiles does
not uninstall Nix or remove its package profiles.

## Ownership and exceptions

Home Manager owns the files named by the generation and its user package
profile. Once its marker exists, legacy `pinst install`, `update`, `apply`,
`bootstrap`, `config apply/adopt`, and `doctor --fix` refuse to compete with
it. `pinst doctor` delegates to `environment status`. Harness management and
tool documentation continue to work independently.

`nix/external-tools.json` explicitly lists OpenCode V2, Claude Code, Aven and
Herdr as external. They are not installed, version-locked, or upgraded by this
environment. Status reports their presence separately from managed-file
health. Keep their existing installations until a reviewed pinned derivation
is added; OpenCode V1 must not substitute for V2. System services, Docker's
daemon, login-shell registration and macOS prerequisites remain host tasks.

## Editor readiness

The Home Manager editor reuses the Lua plugin settings from `configs/nvim/`.
Nix owns plugin acquisition, native fzf, Tree-sitter parsers and query files,
Lua LS, Pyright, and the Copilot language-server artifact. Lazy loads local
store directories with installation disabled; Mason is disabled in this
environment. Completion uses its Lua matcher, avoiding runtime native-library
downloads. The native/legacy editor configuration retains its existing loader.

The Copilot plugin uses the exact commit already recorded in this repo's
`lazy-lock.json`, because the stable Nixpkgs tag archive failed its hash check.
Only `copilot-language-server` is allowed through the unfree-package filter.
Copilot's authenticated service still needs a network connection; editor
startup, syntax parsing and local language-server readiness do not.

## Verification

```sh
just qc
just nix-e2e
```

`nix-e2e` exports tracked working-tree files (stage new files first), builds
pinst and two generations in the digest-pinned Nix container, then tests with
Docker networking disabled. It verifies migration snapshots, Git identity,
no-work repeat activation, package/file ownership, generation rollback, the
doctor JSON contract, shell PATH, plugin paths, native fzf, all configured
parsers and actual Lua LS/Pyright initialization. Its named Docker store volume
is a build cache, not the test home; every activation starts with a fresh home.
Set `PINST_NIX_VOLUME` to choose a different cache volume.

`.github/workflows/nix.yml` runs the container suite on Linux and native
activation/readiness on Apple Silicon and Intel macOS runners. Local Linux
verification also evaluates all four deployment derivations. Installing the
Nix daemon itself is covered by installer fixtures and the macOS CI setup;
the offline Linux lifecycle starts from an image that already contains Nix.

## Updates

```sh
nix flake update
git diff -- flake.lock
pinst environment build --configuration andre-linux --json
pinst environment apply --dry-run --yes --json
pinst environment apply --yes --json
```

Test the new lock before committing/deploying it. Ordinary builds always pass
`--no-update-lock-file`. Installing one package imperatively into the managed
profile creates competing ownership; declare it in `nix/packages.nix` instead.
Package-only environment changes and Home Manager settings share the same
lock. A future release-line change must account for Intel macOS support.
