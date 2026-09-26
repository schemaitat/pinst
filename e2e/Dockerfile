# The test bed `just e2e` boots. Deliberately *only* the toolchain needed to
# build and test pinst, plus one normal non-root account with passwordless
# sudo. Every tool pinst manages is left to the manifest, so the end-to-end
# run installs each one for real on a genuinely fresh machine — bash login
# shell included, so the zsh post-install's chsh step has real work to do.
#
# Ubuntu 24.04, not Debian: the manifest's apt entries name Ubuntu packages
# (git-delta chief among them), so the container must be the distribution the
# manifest actually targets for its apt steps to resolve. Rust is installed
# the way a fresh machine gets it — the rustup sh — because this image is for
# building and testing, not because the manifest says so.
#
# Docker is not installed here on purpose: the suite runs pinst's whole
# lifecycle inside this container, and Docker-in-Docker (a nested daemon)
# does not work — there is nothing for the docker tool to do, and a manifest
# that claimed to manage it would be a test the suite cannot pass.
#
# Build args map the host uid/gid into the image so the mounted checkout
# stays writable; scripts/e2e.sh passes them through.
FROM ubuntu:24.04

ARG HOST_UID=1000
ARG HOST_GID=1000

ENV DEBIAN_FRONTEND=noninteractive

# System deps for the cargo build (gcc/make, cmake for aws-lc-sys, clang for
# the bindgen tree-sitter-cli shells out to) plus what the suite itself
# needs: sudo (pinst's apt steps run under it), python3 (parses the --json
# envelopes the assertions are written against), and the passwd/login
# packages that bring the PAM stack pinst's chsh step goes through.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential pkg-config cmake ninja-build clang libclang-dev \
      ca-certificates curl git sudo python3 passwd login \
 && rm -rf /var/lib/apt/lists/*

# The account the suite runs as: passwordless sudo, a bash login shell (the
# state a real fresh machine is born in), and a uid matching the host so the
# mounted checkout stays writable. ubuntu:24.04 already ships an account at
# uid 1000, so when the host uid is taken, adopt that account (rename it to
# dev) rather than fight over the uid; otherwise create one.
RUN if getent passwd "$HOST_UID" >/dev/null \
      && [ "$(getent passwd "$HOST_UID" | cut -d: -f1)" != root ]; then \
      existing="$(getent passwd "$HOST_UID" | cut -d: -f1)"; \
      existing_group="$(id -gn "$existing")"; \
      usermod --login dev --move-home --home /home/dev --shell /bin/bash "$existing" \
        && groupmod --new-name dev "$existing_group"; \
    else \
      useradd --uid "$HOST_UID" --gid "$HOST_GID" --create-home --shell /bin/bash dev; \
    fi \
 && echo 'dev ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/dev \
 && chmod 440 /etc/sudoers.d/dev

# Rust via the same rustup sh a fresh machine runs, as the root user, into
# the standard /usr/local layout; ownership of cargo/rustup then goes to dev
# so `cargo install` (tree-sitter-cli, just) can place its binaries there.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain stable \
 && chown -R dev:dev /usr/local/rustup /usr/local/cargo \
 && mkdir -p /home/dev/.cargo/registry \
 && chown -R dev:dev /home/dev/.cargo

# The base CARGO_HOME points at /usr/local/cargo, which is also where the
# cargo *binary* lives — mounted over, the toolchain disappears. Point
# CARGO_HOME at the dev home instead, so `just e2e` can mount a cache volume
# over /home/dev/.cargo (registry, cargo-installed binaries, and the
# container-compiled target) without shadowing anything. The toolchain itself
# stays under /usr/local/rustup.
ENV CARGO_HOME=/home/dev/.cargo \
    LIBCLANG_PATH=/usr/lib/llvm-18/lib \
    PATH="/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

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