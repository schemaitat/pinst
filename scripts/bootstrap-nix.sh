#!/usr/bin/env sh
# Install the pinned Nix runtime, then build (never activate) the locked tools.
# Run from a checkout: sh scripts/bootstrap-nix.sh --dry-run
set -eu

dry_run=0
yes=0
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
for arg in "$@"; do
  case "$arg" in
    --dry-run) dry_run=1 ;;
    --yes|-y) yes=1 ;;
    *) echo "usage: $0 [--dry-run] [--yes]" >&2; exit 2 ;;
  esac
done
[ -f "$root/flake.lock" ] || { echo "missing committed flake.lock" >&2; exit 1; }

# Digests are the release-asset SHA-256 values for NixOS/nix-installer 2.35.2.
# The installer bundles Nix itself. Advance this table as one reviewed change.
version=2.35.2
kind=binary
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) target=x86_64-linux; sha=5448a1cd70ad945cb4d36365defbaf3731eba38e23859f3dc8bd7418e1946acc ;;
  Linux/aarch64|Linux/arm64) target=aarch64-linux; sha=a1b35e56da5adadbc117c3cf17b83948ac657f3c0bd79d47bbe0aa70832b5c8e ;;
  Darwin/arm64) target=aarch64-darwin; sha=6314b195321b3acc6826b1c5d66bb9cf9306c8231c6dbb745f51a04c3bcee235 ;;
  Darwin/x86_64) target=x86_64-darwin; kind=script; sha=9adda97297d9e8ab360df95c729eabff4f4f93d6db091953c3a68f29e3fb130c ;;
  *) echo "unsupported platform: $(uname -s)/$(uname -m)" >&2; exit 2 ;;
esac

if ! command -v nix >/dev/null 2>&1; then
  url="https://github.com/NixOS/nix-installer/releases/download/$version/nix-installer-$target"
  # The native installer has no Intel macOS asset. The versioned upstream
  # shell installer verifies its own pinned platform tarball as well.
  [ "$kind" = binary ] || url="https://releases.nixos.org/nix/nix-$version/install"
  printf 'Install Nix with verified installer %s (%s)\n' "$version" "$target"
  if [ "$dry_run" -eq 0 ]; then
    [ "$yes" -eq 1 ] || { echo "re-run with --yes to install Nix" >&2; exit 3; }
    command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
    if [ "$(uname -s)" = Linux ] && [ ! -d /run/systemd/system ]; then
      echo "Enable systemd (including on WSL), or install Nix separately for this host." >&2
      exit 3
    fi
    if [ "$(id -u)" -ne 0 ]; then sudo -n true || { echo "Nix installation requires available sudo credentials" >&2; exit 3; }; fi
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    curl -fsSL --retry 3 --connect-timeout 15 --max-time 300 "$url" -o "$tmp/nix-installer"
    if command -v sha256sum >/dev/null 2>&1; then
      actual=$(sha256sum "$tmp/nix-installer")
    else
      actual=$(shasum -a 256 "$tmp/nix-installer")
    fi
    [ "${actual%% *}" = "$sha" ] || { echo "Nix installer SHA-256 mismatch" >&2; exit 1; }
    if [ "$kind" = binary ]; then
      chmod 755 "$tmp/nix-installer"
      "$tmp/nix-installer" install --no-confirm --enable-flakes
    else
      NIX_INSTALLER_YES=1 sh "$tmp/nix-installer" --daemon --no-channel-add </dev/null
    fi
    # Installed by the verified runtime installer.
    # shellcheck disable=SC1091
    . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
  fi
fi

printf 'Build locked toolchain: %s#toolchain\n' "$root"
printf 'Build pinst from this checkout: %s#pinst\n' "$root"
[ "$dry_run" -eq 0 ] || exit 0
nix --extra-experimental-features 'nix-command flakes' build \
  "$root#toolchain" --no-update-lock-file --out-link "$root/result-toolchain"
mkdir -p "$HOME/.local/state/pinst/environment"
nix --extra-experimental-features 'nix-command flakes' build \
  "$root#pinst" --no-update-lock-file --out-link "$HOME/.local/state/pinst/environment/pinst"
printf 'Toolchain ready at %s/result-toolchain/bin.\n' "$root"
printf 'Use %s/.local/state/pinst/environment/pinst/bin/pinst; see docs/bootstrap.md for activation.\n' "$HOME"
