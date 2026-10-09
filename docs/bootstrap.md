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
pinst environment build --flake . --configuration andre-linux --dry-run --json
pinst environment build --flake . --configuration andre-linux --json
pinst environment apply --dry-run --yes --json
pinst environment apply --yes --json
pinst environment status --json
```

`bootstrap-nix.sh` downloads the pinned NixOS installer and verifies its
committed SHA-256 before execution. It builds the package-only toolchain, with
no home activation. Intel macOS needs an existing Nix installation: this
installer release has no Intel asset. Linux/WSL installation expects systemd
and available sudo credentials. Existing Nix installations are reused.

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
