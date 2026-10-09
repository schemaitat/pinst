#!/usr/bin/env bash
# Run only with built store paths in a disposable container/CI account.
set -euo pipefail
: "${PINST_BIN:?path to built pinst}"
: "${GENERATION:?first Home Manager generation}"
: "${NEXT_GENERATION:?second Home Manager generation}"
export HOME=/tmp/pinst-home
USER="$(id -un)"
export USER
[[ ! -e "$HOME" ]] || { echo "$HOME must be absent for the fresh-home test" >&2; exit 1; }
mkdir -p "$HOME/dotfiles"
printf '# preserved live edit\n' > "$HOME/dotfiles/zshrc"
ln -s dotfiles/zshrc "$HOME/.zshrc"
printf '[user]\n name = Test User\n email = test@example.invalid\n' > "$HOME/.gitconfig"
python="$GENERATION/home-path/bin/python"

"$PINST_BIN" environment apply --generation "$GENERATION" --migrate --dry-run --yes --json > /tmp/nix-preview.json
[[ ! -e "$HOME/.local/state/pinst" ]]
[[ "$(readlink "$HOME/.zshrc")" == dotfiles/zshrc ]]
set +e
"$PINST_BIN" environment apply --generation "$GENERATION" --yes --json > /tmp/nix-blocked.json
rc=$?
set -e
[[ $rc == 3 ]]
"$PINST_BIN" environment apply --generation "$GENERATION" --migrate --yes --json > /tmp/nix-first.json
"$PINST_BIN" environment status --json > /tmp/nix-status.json
"$PINST_BIN" environment apply --generation "$GENERATION" --yes --json > /tmp/nix-repeat.json
"$python" - <<'PY'
import json, os, pathlib
home = pathlib.Path(os.environ['HOME'])
receipt, = (home / '.local/state/pinst/environment/migrations').glob('*/receipt.json')
backup = receipt.parent
assert (backup / 'snapshot/.zshrc').read_text() == '# preserved live edit\n'
assert os.readlink(backup / 'original/.zshrc') == 'dotfiles/zshrc'
assert (home / 'dotfiles/zshrc').read_text() == '# preserved live edit\n'
assert 'Test User' in (home / '.config/pinst/git-identity').read_text()
repeat = json.load(open('/tmp/nix-repeat.json'))['summary']['execution']
assert repeat['ran'] == 0 and repeat['skipped'] == 1, repeat
assert json.load(open('/tmp/nix-status.json'))['summary']['managed_healthy']
PY
set +e
"$PINST_BIN" config apply --dry-run --json > /tmp/nix-owner.json
rc=$?
set -e
[[ $rc == 2 ]]
"$PINST_BIN" environment apply --generation "$NEXT_GENERATION" --yes --json > /tmp/nix-next.json
[[ "$(readlink "$HOME/.local/state/pinst/environment/current")" == "$NEXT_GENERATION" ]]
"$PINST_BIN" environment rollback --dry-run --yes --json > /tmp/nix-rollback-preview.json
"$PINST_BIN" environment rollback --yes --json > /tmp/nix-rollback.json
[[ "$(readlink "$HOME/.local/state/pinst/environment/current")" == "$GENERATION" ]]
"$PINST_BIN" doctor --json > /tmp/nix-doctor.json
"$python" - <<'PY'
import json
doctor = json.load(open('/tmp/nix-doctor.json'))
assert doctor['command'] == 'doctor', doctor
assert all(item['severity'] == 'info' for item in doctor['items']), doctor
PY
# The child zsh must expand these expressions.
# shellcheck disable=SC2016
"$HOME/.nix-profile/bin/zsh" -lic 'set -e; for tool in rg fd nvim node python uv just gh delta; do command -v "$tool"; done; [[ "$(git config user.name)" == "Test User" ]]'
"$HOME/.nix-profile/bin/nvim" --headless "+luafile $(dirname "$0")/nvim-smoke.lua"
echo 'Nix lifecycle: migration, repeat activation, ownership and rollback passed'
