#!/usr/bin/env bash
set -euo pipefail

# The e2e suite — runs *inside* the container `just e2e` boots, against the
# checkout mounted at /src. Every phase a fresh machine experiences happens
# here, in order:
#
#   0. preflight   the image is a clean slate: no managed tool is installed
#   1. tests       cargo test --locked — the same gate CI runs
#   2. install     scripts/install.sh places the binary, as a fresh machine
#                  would receive it
#   3. values      machine-specific template values, as provisioning expects
#   4. plan        pinst bootstrap --dry-run, read before anything mutates
#   5. bootstrap   pinst bootstrap -y — install every tool, apply the configs,
#                  chsh to zsh — from a scratch dir, so the *embedded*
#                  manifest and configs drive it like a downloaded binary
#   6. idempotent  bootstrap again: ran == 0 — nothing left to do
#   7. doctor      no error-severity finding may remain
#
# Arguments are forwarded as a pinst selection, so `just e2e --profile
# minimal` (or `just e2e zsh oh-my-zsh`) narrows the bootstrap; the default
# is the full `default` profile.
#
# docker-in-docker does not work, so there is no docker tool in play — the
# container provisions whatever the manifest declares, and nothing more.

src=/src
home=/home/dev
export HOME="$home"

# The checkout is mounted read-write at /src, but its target/ must NOT be
# shared: this host's target/ holds binaries linked against a newer glibc
# than the container's, and cargo inside the container treats them as fresh
# and tries to run them. Point cargo at a container-local target so the
# container always compiles its own artifacts. The ~/.cargo cache volume
# persists only the crate *registry* (downloads, which is what makes repeat
# runs cheap); the target dir and any cargo-installed binaries live in the
# ephemeral container, so every run is still a genuinely fresh machine, not
# the previous run's leftovers.
export CARGO_TARGET_DIR="$home/.cargo/target"

selection=("$@")

step() { printf '\n\033[1;36m== [%s] %s\033[0m\n' "$(date +%H:%M:%S)" "$*"; }
fail() { printf '\033[1;31mxx %s\033[0m\n' "$*" >&2; exit 1; }

# pinst's probes run `sh -c` with the inherited PATH, so it must look like a
# configured machine's: nvm's node dir, cargo and user-local bins where the
# installers place things — including ~/.opencode/bin, where the opencode
# installer lands (it does not go to ~/.local/bin).
refresh_path() {
  local path="" dir
  for dir in "$home"/.nvm/versions/node/*/bin \
             /usr/local/cargo/bin \
             "$home/.cargo/bin" \
             "$home/.local/bin" \
             "$home/.opencode/bin" \
             /usr/local/bin /usr/bin /bin /usr/sbin /sbin; do
    if [ -d "$dir" ]; then
      path="${path:+$path:}$dir"
    fi
  done
  export PATH="$path"
}

# Exit 0 or 3 are both "ran fine": 3 is pinst's documented "there is
# something to look at" verdict (a `manual` tool, an `unsupported` platform
# note). 1 means a step failed and 2 a usage error — those fail the suite.
tolerate_ok_or_issues() {
  local name="$1" rc="$2"
  [ "$rc" -eq 0 ] || [ "$rc" -eq 3 ] \
    || fail "$name exited $rc (expected 0 or 3)"
}

cd "$src"
refresh_path

step "phase 0: preflight — the image must be a clean slate"
for tool in zsh rg fd nvim tree-sitter uv node gh delta direnv oh-my-posh just \
    git-credential-manager herdr aven opencode claude pinst; do
  command -v "$tool" >/dev/null 2>&1 && fail "managed tool '$tool' is already installed by the image"
done
[ -d "$home/.oh-my-zsh" ] && fail "oh-my-zsh is already installed by the image"
[ -d "$home/.nvm" ] && fail "nvm is already installed by the image"
printf '        clean slate: ok\n'

step "phase 1: cargo test --locked"
cargo test --locked

step "phase 2: install pinst via scripts/install.sh (builds this checkout)"
refresh_path
PINST_INSTALL_DIR="$home/.local/bin" scripts/install.sh
# install.sh just created ~/.local/bin; refresh PATH so pinst resolves there.
refresh_path
command -v pinst >/dev/null || fail "pinst is not on PATH after install"
printf '        pinst %s\n' "$(pinst --version)"

step "phase 3: machine-specific values (the git config template needs them)"
refresh_path
values="$home/.config/pinst/values.toml"
mkdir -p "$(dirname "$values")"
cat > "$values" <<'EOF'
GIT_USER_NAME = "pinst e2e"
GIT_USER_EMAIL = "e2e@example.invalid"
EOF

scratch="$(mktemp -d)"
step "phase 4: pinst bootstrap --dry-run from a scratch dir (embedded manifest + configs)"
refresh_path
cd "$scratch"
set +e
# -y with --dry-run is not redundant: the confirm gate runs before the
# dry-run check, so the preview shows every step as "would run".
pinst bootstrap --dry-run -y "${selection[@]}"
rc=$?
set -e
tolerate_ok_or_issues "bootstrap --dry-run" "$rc"

step "phase 5: pinst bootstrap -y (install tools, apply configs, chsh to zsh)"
# Exit 3 is expected even after a perfect run: bootstrap's closing re-probe
# shares its process' PATH, which was captured before tools like node and
# opencode created their PATH dirs mid-run. A fresh `pinst` process (phase 6
# and phase 7, after refresh_path) is where "is everything really installed"
# gets an honest answer.
refresh_path
set +e
pinst bootstrap -y "${selection[@]}"
rc=$?
set -e
tolerate_ok_or_issues "bootstrap -y" "$rc"

step "phase 6: idempotency — a second bootstrap must have nothing left to do"
refresh_path
set +e
pinst bootstrap -y --json "${selection[@]}" > /tmp/boot2.json 2>/dev/null
rc=$?
set -e
tolerate_ok_or_issues "second bootstrap" "$rc"
python3 - <<'PY'
import json, sys
d = json.load(open('/tmp/boot2.json'))
run = d['summary']['execution']
print(f"        second run: {run['ran']} ran, {run['skipped']} skipped, "
      f"{run['failed']} failed, {run['needs_confirmation']} need authorization")
if run['ran'] != 0 or run['failed'] != 0 or run['needs_confirmation'] != 0:
    print(json.dumps(d, indent=2))
    sys.exit(1)
PY

step "phase 7: pinst doctor — no error-severity finding may remain"
refresh_path
set +e
pinst doctor --json "${selection[@]}" > /tmp/doctor.json 2>/dev/null
rc=$?
set -e
tolerate_ok_or_issues "doctor" "$rc"
python3 - <<'PY'
import json, sys
d = json.load(open('/tmp/doctor.json'))
errs = [i for i in d['items'] if i['severity'] == 'error']
warns = [i for i in d['items'] if i['severity'] == 'warning']
infos = [i for i in d['items'] if i['severity'] == 'info']
for i in warns + infos:
    print(f"        [{i['severity']}] {i['id']}: {i['message']}")
print(f"        {len(infos)} info, {len(warns)} warning, {len(errs)} error")
if errs:
    for i in errs:
        print(f"        [error] {i['id']}: {i['message']}", file=sys.stderr)
    sys.exit(1)
print("        doctor: in sync (only info/warning findings remain)")
PY

step "phase 8: pinst list — the provisioned machine at a glance"
refresh_path
set +e
pinst list "${selection[@]}"
rc=$?
set -e
tolerate_ok_or_issues "list" "$rc"

step "e2e complete — the whole lifecycle verified against a fresh machine"