#!/usr/bin/env sh
# Fresh-machine entry point for pinst.
#
#   curl -fsSL https://raw.githubusercontent.com/schemaitat/pinst/main/scripts/install.sh | sh
#
# Installs the released binary to ~/.local/bin — no clone, no Rust toolchain,
# no build. The binary carries the manifest and the configs, so after this
# runs a single `pinst bootstrap` provisions the whole machine.
#
# Building from source stays as the fallback, and as the path taken when this
# script runs from inside a pinst checkout, which is what `just install`
# does: a developer asking to install their working tree must not silently
# get the last release instead.
#
# Environment:
#   PINST_INSTALL_DIR       where the binary goes (default ~/.local/bin)
#   PINST_VERSION           pin a release tag, e.g. v0.2.0 (default: latest)
#   PINST_BUILD_FROM_SOURCE 1 to skip the download and compile instead
#   PINST_REPO_SLUG         owner/name to fetch releases from
#   PINST_RELEASE_BASE      override the release URL base (a mirror, or a
#                           local server when testing this script)
#   PINST_REPO              git URL for the source fallback
#   PINST_SRC_DIR           where the source fallback clones to
set -eu

SLUG="${PINST_REPO_SLUG:-schemaitat/pinst}"
REPO_URL="${PINST_REPO:-https://github.com/$SLUG.git}"
INSTALL_DIR="${PINST_INSTALL_DIR:-$HOME/.local/bin}"
SRC_DIR="${PINST_SRC_DIR:-$HOME/.local/share/pinst/src}"
VERSION="${PINST_VERSION:-}"

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
err() { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; }

# The release target for this machine's `uname -s`/`uname -m`, or empty when
# none is built. A lookup table, not a formula: `uname -m` reports `arm64`
# on Apple Silicon, not `aarch64` the way the Rust target triple spells it,
# so the two have to be mapped rather than assembled.
detect_target() {
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os/$arch" in
    Linux/x86_64) echo "x86_64-unknown-linux-musl" ;;
    Darwin/arm64) echo "aarch64-apple-darwin" ;;
    Darwin/x86_64) echo "x86_64-apple-darwin" ;;
    *) return 1 ;;
  esac
}

in_checkout() {
  [ -f "Cargo.toml" ] && grep -q '^name = "pinst"' Cargo.toml 2>/dev/null
}

place() {
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$1" "$INSTALL_DIR/pinst"
  log "installed $INSTALL_DIR/pinst"
}

# Verifies the download against the checksum published beside it. A tool that
# refuses to pretend `curl | sh` installers are verifiable has no business
# installing itself unverified.
verify() {
  dir="$1"; asset="$2"
  if command -v sha256sum >/dev/null 2>&1; then
    ( cd "$dir" && sha256sum -c "$asset.sha256" >/dev/null 2>&1 )
  elif command -v shasum >/dev/null 2>&1; then
    ( cd "$dir" && shasum -a 256 -c "$asset.sha256" >/dev/null 2>&1 )
  else
    err "no sha256 tool available to verify the download"
    return 1
  fi
}

# Returns non-zero when there is nothing to download (wrong platform, no
# release, no network) so the caller can fall back to building. A download
# that arrives but fails verification is different: that is a hard stop.
download_release() {
  target="$(detect_target)" || {
    log "no prebuilt binary for $(uname -s)/$(uname -m)"
    return 1
  }
  command -v curl >/dev/null 2>&1 || { log "curl not found"; return 1; }
  command -v tar >/dev/null 2>&1 || { log "tar not found"; return 1; }

  asset="pinst-$target.tar.gz"
  if [ -n "${PINST_RELEASE_BASE:-}" ]; then
    base="$PINST_RELEASE_BASE"
  elif [ -n "$VERSION" ]; then
    base="https://github.com/$SLUG/releases/download/$VERSION"
  else
    base="https://github.com/$SLUG/releases/latest/download"
  fi

  tmp="$(mktemp -d)"
  if ! curl -fsSL -o "$tmp/$asset" "$base/$asset" \
     || ! curl -fsSL -o "$tmp/$asset.sha256" "$base/$asset.sha256"; then
    log "no release asset at $base/$asset"
    rm -rf "$tmp"
    return 1
  fi

  if ! verify "$tmp" "$asset"; then
    err "checksum mismatch for $asset — refusing to install it"
    err "re-run with PINST_BUILD_FROM_SOURCE=1 to build instead"
    rm -rf "$tmp"
    exit 1
  fi
  log "verified $asset"

  tar -C "$tmp" -xzf "$tmp/$asset"
  place "$tmp/pinst"
  rm -rf "$tmp"
}

build_from_source() {
  if in_checkout; then
    SRC_DIR="$(pwd)"
    log "building from the current checkout: $SRC_DIR"
  elif [ -d "$SRC_DIR/.git" ]; then
    log "updating $SRC_DIR"
    git -C "$SRC_DIR" pull --ff-only
  else
    log "cloning $REPO_URL into $SRC_DIR"
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --depth=1 "$REPO_URL" "$SRC_DIR"
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    log "installing rust (needed to build pinst itself)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  fi

  log "building release binary"
  ( cd "$SRC_DIR" && cargo build --release --locked )
  place "$SRC_DIR/target/release/pinst"
}

if in_checkout; then
  build_from_source
elif [ "${PINST_BUILD_FROM_SOURCE:-}" = "1" ]; then
  build_from_source
elif ! download_release; then
  log "falling back to building from source"
  build_from_source
fi

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) log "add $INSTALL_DIR to your PATH to use pinst" ;;
esac

cat <<'NEXT'

Next:
  pinst bootstrap --dry-run    # see exactly what would happen
  pinst bootstrap -y           # provision this machine
  pinst doctor                 # check the result
NEXT
