# Development tasks for working *on* pinst.
#
# (Not to be confused with the ~/dotfiles justfile pinst replaced — that one
# drove GNU Stow and is retired. This one only builds and checks this repo.)

# Show available recipes
default:
    @just --list

# With no args this opens the dashboard. Exit codes propagate, so `just run
# doctor` reporting "recipe failed with exit code 3" is pinst saying it found
# issues — that is its documented contract, not a broken recipe.
[doc("Run pinst from source; args forward (`just run doctor --json`)")]
run *ARGS="tui":
    cargo run --quiet -- {{ ARGS }}

# Every quality check, in the order that fails fastest.
qc: fmt-check lint test

# Fail if the tree is not formatted.
fmt-check:
    cargo fmt --check

# Lint, treating warnings as errors so they cannot accumulate.
lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test

# Reformat the tree in place.
fmt:
    cargo fmt

# The self-contained release binary, the artifact that gets shipped.
build:
    cargo build --release

# Delegates to scripts/install.sh rather than repeating the build-and-place
# steps, so there is one answer to "how does pinst get installed" — the same
# one a fresh machine runs.
[doc("Build and put pinst on PATH (default ~/.local/bin; pass a dir to override)")]
install dir="":
    PINST_INSTALL_DIR="{{ dir }}" scripts/install.sh
