#!/usr/bin/env bash
# Build from tracked working-tree bytes, then test activation with no network.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
image=nixos/nix@sha256:7a007c766426c1877758ddc5cb87a965ac131fc78c582ce0083d922d51ae945c
volume=${PINST_NIX_VOLUME:-pinst-nix-e2e-store}
cd "$root"
# A linked worktree's .git points outside the checkout. Export tracked files
# so the test input is portable and excludes target/ and untracked secrets.
git ls-files -z | tar --null -T - -cf "$tmp/source.tar"
docker run --rm --volume "$volume:/nix" --volume "$tmp:/input" "$image" bash -c '
  set -euo pipefail
  mkdir -p /src
  tar -xf /input/source.tar -C /src
  cd /src
  nix --extra-experimental-features "nix-command flakes" build path:/src#pinst --no-update-lock-file --out-link /tmp/pinst
  /tmp/pinst/bin/pinst environment build --flake /src --configuration test-linux --out-link /tmp/base --json
  /tmp/pinst/bin/pinst environment build --flake /src --configuration test-linux-next --out-link /tmp/next --json
  printf "PINST_BIN=%s/bin/pinst\nGENERATION=%s\nNEXT_GENERATION=%s\n" "$(readlink -f /tmp/pinst)" "$(readlink -f /tmp/base)" "$(readlink -f /tmp/next)" > /input/paths.env
'
docker run --rm --network none --volume "$volume:/nix" --volume "$tmp:/input:ro" \
  --env-file "$tmp/paths.env" "$image" bash -c '
  set -euo pipefail
  mkdir -p /src
  tar -xf /input/source.tar -C /src
  bash /src/e2e/nix-lifecycle.sh
'
