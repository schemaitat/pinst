#!/usr/bin/env bash
set -euo pipefail

# Drives the end-to-end suite: builds the container image and runs the whole
# pinst lifecycle inside it — unit/integration tests, install, bootstrap,
# doctor — against this checkout, mounted at /src. Nothing on the host is
# touched beyond the gitignored target/ (and a docker cache volume).
#
#   scripts/e2e.sh                      full default-profile bootstrap
#   scripts/e2e.sh --profile minimal    a quicker shell-only pass
#   scripts/e2e.sh zsh oh-my-zsh        just a couple of tools
#
# Environment:
#   PINST_E2E_IMAGE    image tag (default pinst-e2e)
#   PINST_E2E_CACHE    named volume for ~/.cargo registry+bin cache across
#                      runs (default pinst-e2e-cargo-<uid>)

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
image="${PINST_E2E_IMAGE:-pinst-e2e}"
cache="${PINST_E2E_CACHE:-pinst-e2e-cargo-$(id -u)}"

# The image maps the caller's uid/gid into the dev account so the mounted
# checkout is owned by the same user the suite runs as. Root cannot map onto
# a fresh user — and docker-in-docker does not work, so running the suite as
# root buys nothing anyway.
if [ "$(id -u)" -eq 0 ]; then
  echo "xx run the e2e suite as a non-root user so its uid can map into the container" >&2
  exit 1
fi

docker build \
  --build-arg "HOST_UID=$(id -u)" \
  --build-arg "HOST_GID=$(id -g)" \
  -t "$image" \
  -f "$root/e2e/Dockerfile" \
  "$root"

# target/ persists through the mount, keeping rebuilds incremental; the
# named volume keeps the crate registry and cargo-installed binaries between
# runs. The container is throwaway, so every pinst mutation lands in
# ephemeral state and vanishes when the run ends.
exec docker run --rm \
  --name "pinst-e2e-$$" \
  -v "$root":/src \
  -v "$cache":/home/dev/.cargo \
  "$image" \
  /src/e2e/run.sh "$@"