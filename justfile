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
qc: fmt-check lint test harness

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

# The release artifact, exactly as CI builds it: one statically linked binary
# in a tarball, plus the checksum scripts/install.sh verifies before trusting
# a download.
#
# The release workflow calls this same recipe, so there is one answer to "how
# is a release artifact built" — the way scripts/install.sh is the one answer
# to "how does pinst get installed". The tarball name carries no version: the
# github_release install method resolves
# /releases/latest/download/<asset>, which is a fixed path.
[doc("Build the release tarball for a target into dist/ (default: static musl)")]
dist target="x86_64-unknown-linux-musl":
    #!/usr/bin/env bash
    set -euo pipefail
    target="{{ target }}"
    host="$(rustc -vV | sed -n 's/^host: //p')"
    # Same architecture, different libc, is something the local toolchain
    # handles: the musl build needs only `musl-tools` and `cmake` installed
    # (cmake for aws-lc-rs, rustls' crypto provider via reqwest). cross is
    # reserved for a genuinely foreign architecture, where a whole
    # cross-toolchain in a container is the only sane way.
    if [ "${target%%-*}" = "${host%%-*}" ]; then
        rustup target add "$target"
        cargo build --release --locked --target "$target" \
            || { echo "hint: the musl build needs musl-tools and cmake" >&2; exit 1; }
    else
        cross build --release --locked --target "$target"
    fi
    rm -rf dist && mkdir -p dist
    # --sort/--owner/--group keep the archive identical whoever builds it, so
    # a locally built artifact can be compared against the released one.
    tar --sort=name --owner=0 --group=0 --numeric-owner \
        -C "target/$target/release" -czf "dist/pinst-$target.tar.gz" pinst
    cd dist && sha256sum "pinst-$target.tar.gz" > "pinst-$target.tar.gz.sha256"
    ls -l "pinst-$target.tar.gz" "pinst-$target.tar.gz.sha256"

# --- the agent harness -----------------------------------------------------
# Source of truth is .agents/ (skills) and .ash/ (the plan corpus). These
# recipes are what keeps both honest; see .agents/README.md.

# Without this the skills in .agents/ are inert: no runtime reads that path.
[doc("Wire .agents/skills into .claude/skills; run after adding or renaming one")]
wire:
    scripts/agents-wire.sh

[doc("Regenerate .ash/INDEX.md from the plan frontmatter (never hand-edit it)")]
index:
    scripts/ash.sh index

# Exit code 3 means "found things to act on", the same verdict pinst itself
# gives — so this fails `qc` until the corpus is clean again.
[doc("Validate the harness: skills wired, plan corpus consistent")]
harness:
    scripts/agents-wire.sh --check
    scripts/ash.sh check
