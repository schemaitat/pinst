#!/usr/bin/env bash
# For disposable macOS CI runners with Nix already installed.
set -euo pipefail
case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) config=test-macos ;;
  Darwin/x86_64) config=test-macos-intel ;;
  *) echo 'Use scripts/nix-e2e.sh for Linux container verification' >&2; exit 2 ;;
esac
nix --extra-experimental-features 'nix-command flakes' build .#pinst --no-update-lock-file --out-link result-pinst
./result-pinst/bin/pinst environment build --flake . --configuration "$config" --out-link result-home --json
./result-pinst/bin/pinst environment build --flake . --configuration "$config-next" --out-link result-next --json
export PINST_BIN="$PWD/result-pinst/bin/pinst"
GENERATION="$(cd result-home && pwd -P)"
NEXT_GENERATION="$(cd result-next && pwd -P)"
export GENERATION NEXT_GENERATION
bash e2e/nix-lifecycle.sh
