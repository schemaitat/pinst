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
#
# `--locked` here and in `test` mirrors CI exactly. Without it a stale
# Cargo.lock passes locally *and gets silently rewritten*, while CI — which
# has always passed --locked — fails. That is precisely the tree
# release-please's release PR produces, so the two must not disagree.
lint:
    cargo clippy --all-targets --locked -- -D warnings

test:
    cargo test --locked

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
    # Compare vendor+OS, not the whole triple and not just the leading arch
    # field. Same architecture, different libc (x86_64-unknown-linux-gnu
    # host building the -musl target) is something the local toolchain
    # handles, which is why this cannot compare the full triple — musl vs
    # gnu would wrongly read as "foreign". Same OS, different architecture
    # (an arm64 Mac building x86_64-apple-darwin, or vice versa) is also
    # something Apple's own toolchain cross-compiles between natively,
    # which is why this cannot compare only the arch field either — that
    # was the bug: an arm64 Mac building x86_64-apple-darwin used to fail
    # this check and fall to the `cross` branch below, which has no Apple
    # support and is not installed on the release runner. `cross` and its
    # container are reserved for a genuinely foreign OS.
    IFS='-' read -r _ target_vendor target_os _ <<< "$target"
    IFS='-' read -r _ host_vendor host_os _ <<< "$host"
    if [ "$target_vendor-$target_os" = "$host_vendor-$host_os" ]; then
        rustup target add "$target"
        cargo build --release --locked --target "$target" \
            || { echo "hint: the musl build needs musl-tools and cmake" >&2; exit 1; }
    else
        cross build --release --locked --target "$target"
    fi
    rm -rf dist && mkdir -p dist
    # --sort/--owner/--group keep the archive identical whoever builds it, so
    # a locally built artifact can be compared against the released one. BSD
    # tar (what macOS ships) rejects all four flags outright, so this only
    # runs them under GNU tar — `gtar` when present (a Homebrew install),
    # falling back to `tar` when that GNU tar is what `tar` itself resolves
    # to (Linux, or a Mac with coreutils' gnubin on PATH). A Mac building
    # with neither loses only the reproducibility property, not the build:
    # the archive's *contents* are identical either way, just not
    # byte-identical to one built elsewhere.
    if command -v gtar >/dev/null 2>&1; then
        GNUTAR=gtar
    elif tar --version 2>/dev/null | grep -q GNU; then
        GNUTAR=tar
    else
        GNUTAR=""
    fi
    if [ -n "$GNUTAR" ]; then
        "$GNUTAR" --sort=name --owner=0 --group=0 --numeric-owner \
            -C "target/$target/release" -czf "dist/pinst-$target.tar.gz" pinst
    else
        echo "note: no GNU tar found — dist/pinst-$target.tar.gz will not be byte-reproducible" >&2
        tar -C "target/$target/release" -czf "dist/pinst-$target.tar.gz" pinst
    fi
    cd dist
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "pinst-$target.tar.gz" > "pinst-$target.tar.gz.sha256"
    else
        # macOS has no sha256sum; shasum -a 256 produces the same line
        # format, so scripts/install.sh's `shasum -a 256 -c` fallback reads
        # either file the same way.
        shasum -a 256 "pinst-$target.tar.gz" > "pinst-$target.tar.gz.sha256"
    fi
    ls -l "pinst-$target.tar.gz" "pinst-$target.tar.gz.sha256"

# --- the agent harness -----------------------------------------------------
# Source of truth is .agents/ (skills) and .ash/ (the plan corpus). These
# recipes are what keeps both honest; see .agents/README.md.
#
# The corpus half is `pinst harness` now, not a script: a machine that
# installed the release binary gets the same checks with nothing to clone.
# Skill projection is still bash, because `agents-wire.sh` writes symlinks
# into a vendor directory and has nothing to do with the corpus.

# Without this the skills in .agents/ are inert: no runtime reads that path.
[doc("Wire .agents/skills into .claude/skills; run after adding or renaming one")]
wire:
    scripts/agents-wire.sh

# Needs a build now that the generator lives in the binary. Inside `qc` that
# is free — `harness` runs after `test`, so the tree is already compiled — but
# a bare `just index` on a cold checkout pays for it once.
[doc("Regenerate .ash/INDEX.md from the plan frontmatter (never hand-edit it)")]
index:
    cargo run --quiet -- harness index

# Exit code 3 means "found things to act on", the same verdict pinst itself
# gives — so this fails `qc` until the corpus is clean again.
[doc("Validate the harness: skills wired, plan corpus consistent")]
harness:
    scripts/agents-wire.sh --check
    cargo run --quiet -- harness check

# The review pass. Deliberately NOT in `qc`: `skills` reports rates and
# tallies, and report() exits 3 on a finding of any severity, so a number
# that got interesting would fail the build and the check would get deleted.
# Invariants gate commits; measurements start conversations.
#
# Both halves always run — a failing `check` must not hide the report — and
# the worst exit code wins, so 3 still means "there is something to act on".
[doc("The harness review: corpus invariants, then the graded per-skill report")]
review:
    #!/usr/bin/env bash
    rc=0
    cargo run --quiet -- harness check || rc=$?
    cargo run --quiet -- harness skills || rc=$?
    exit $rc

# The rails around the unattended distillation in .github/workflows/distil.yml.
# Same name here and in CI, so a maintainer debugging a scheduled run
# reproduces it exactly rather than approximating it.
#
#   just distil-guard preflight   0 there is work, 1 corpus clean
#   just distil-guard verify      0 safe to publish, 3 findings to act on
[doc("Gate and guard for the scheduled distillation (preflight | verify)")]
distil-guard *ARGS:
    scripts/distil-guard.sh {{ ARGS }}
