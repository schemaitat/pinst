#!/usr/bin/env sh
# Fresh-machine entry point for pinst.
#
#   curl -fsSL https://raw.githubusercontent.com/<owner>/pinst/main/scripts/install.sh | sh
#
# Builds pinst from this repo (cloning it first if needed) and installs the
# binary to ~/.local/bin. The binary carries the manifest and the configs, so
# after this runs a single `pinst bootstrap` provisions the whole machine.
set -eu

REPO_URL="${PINST_REPO:-}"
INSTALL_DIR="${PINST_INSTALL_DIR:-$HOME/.local/bin}"
SRC_DIR="${PINST_SRC_DIR:-$HOME/.local/share/pinst/src}"

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
err() { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; }

if [ -f "Cargo.toml" ] && grep -q '^name = "pinst"' Cargo.toml 2>/dev/null; then
  SRC_DIR="$(pwd)"
  log "building from the current checkout: $SRC_DIR"
elif [ -n "$REPO_URL" ]; then
  if [ -d "$SRC_DIR/.git" ]; then
    log "updating $SRC_DIR"
    git -C "$SRC_DIR" pull --ff-only
  else
    log "cloning $REPO_URL into $SRC_DIR"
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --depth=1 "$REPO_URL" "$SRC_DIR"
  fi
else
  err "run this from a pinst checkout, or set PINST_REPO=<git url>"
  exit 2
fi

if ! command -v cargo >/dev/null 2>&1; then
  log "installing rust (needed to build pinst itself)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

log "building release binary"
( cd "$SRC_DIR" && cargo build --release --locked )

mkdir -p "$INSTALL_DIR"
install -m 0755 "$SRC_DIR/target/release/pinst" "$INSTALL_DIR/pinst"
log "installed $INSTALL_DIR/pinst"

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
