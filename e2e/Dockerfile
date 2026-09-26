# The test bed `just e2e` boots. Deliberately *only* the toolchain needed to
# build and test pinst, plus one normal non-root account with passwordless
# sudo. Every tool pinst manages is left to the manifest, so the end-to-end
# run installs each one for real on a genuinely fresh machine — bash login
# shell included, so the zsh post-install's chsh step has real work to do.
#
# Docker is not installed here on purpose: the suite runs pinst's whole
# lifecycle inside this container, and Docker-in-Docker (a nested daemon)
# does not work — there is nothing for the docker tool to do, and a manifest
# that claimed to manage it would be a test the suite cannot pass.
#
# Build args map the host uid/gid into the image so the mounted checkout
# stays writable; scripts/e2e.sh passes them through.
FROM rust:1-bookworm

ARG HOST_UID=1000
ARG HOST_GID=1000

ENV DEBIAN_FRONTEND=noninteractive

# The rust base image already carries gcc/make (build-essential), pkg-config,
# curl, git and ca-certificates for the cargo build. The only additions the
# suite needs: sudo (pinst's apt steps run under it), python3 (parses the
# --json envelopes the assertions are written against), and the passwd/login
# packages that bring the PAM stack pinst's chsh step goes through.
RUN apt-get update \
 && apt-get install -y --no-install-recommends sudo python3 passwd login \
 && rm -rf /var/lib/apt/lists/*

# The account the suite runs as. Passwordless sudo, a bash login shell (the
# state a real fresh machine is born in), and ownership of the rust install
# so `cargo install` (tree-sitter-cli, just) can place its binaries there.
RUN groupadd --gid "$HOST_GID" dev \
 && useradd --uid "$HOST_UID" --gid "$HOST_GID" --create-home \
      --shell /bin/bash dev \
 && echo 'dev ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/dev \
 && chmod 440 /etc/sudoers.d/dev \
 && chown -R dev:dev /usr/local/cargo /usr/local/rustup \
 && mkdir -p /home/dev/.cargo && chown -R dev:dev /home/dev/.cargo

# The base image's CARGO_HOME points at /usr/local/cargo, which is also where
# the cargo *binary* lives — mounted over, the toolchain disappears. Point
# CARGO_HOME at the dev home instead, so `just e2e` can mount a cache volume
# over /home/dev/.cargo (registry + cargo-installed binaries) without
# shadowing anything. The toolchain itself stays under /usr/local/rustup.
ENV CARGO_HOME=/home/dev/.cargo

# A running container has no controlling terminal, so PAM's password prompt
# would hang — and then fail — the userland `chsh` pinst's zsh post-install
# runs. This is a throwaway test bed, so chsh's PAM auth is stubbed to always
# pass. The step pinst runs is the real `chsh -s "$(command -v zsh)"`, with
# the same /etc/shells validation and skip_if logic; only the password dance
# is removed.
RUN printf '%s\n' \
      'auth sufficient pam_permit.so' \
      'auth sufficient pam_unix.so nullok' \
      'auth required pam_deny.so' \
      'account required pam_permit.so' \
      'session required pam_permit.so' \
      > /etc/pam.d/chsh

USER dev
ENV HOME=/home/dev

# Where `just e2e` mounts this checkout.
WORKDIR /src