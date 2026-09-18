[ -s "$HOME/.cargo/env" ] && \. "$HOME/.cargo/env"

# uv
export PATH="/Users/andre/.local/bin:$PATH"

# nvm (sourced here too so non-interactive shells, e.g. tool-invoked commands, get node/npm/npx on PATH)
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
