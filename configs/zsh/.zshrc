# Path to your Oh My Zsh installation.
export ZSH="$HOME/.oh-my-zsh"

export PATH="$HOME/.local/bin:$PATH"
export PATH="$HOME/.opencode/bin:/opt/nvim-linux-x86_64/bin:$PATH"

ZSH_THEME=""

HYPHEN_INSENSITIVE="true"
COMPLETION_WAITING_DOTS="true"

zstyle ':omz:update' mode auto  # update automatically without asking

plugins=(
  git
  z
  sudo
  colored-man-pages
  zsh-autosuggestions
  direnv
)

source $ZSH/oh-my-zsh.sh

# --- Completion ---
# compinit already runs inside oh-my-zsh.sh (with caching + async security
# check); calling it again here just redoes the full scan synchronously.

zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'  # case-insensitive
zstyle ':completion:*' list-colors "${(s.:.)LS_COLORS}"
zstyle ':completion:*' list-suffixes
zstyle ':completion:*' expand prefix suffix
zstyle ':completion:*' menu select
zstyle ':completion:*' use-cache on
zstyle ':completion:*' cache-path ~/.zsh/cache

# --- aliases ---

alias clauded="claude --dangerously-skip-permissions"
alias v="nvim"

# --- opencode ---

alias t="task"
alias rn="ronin"
alias oc="opencode"
alias oca="ronin sandbox run --otel --herdr opencode --auto"
alias ocr="ronin sandbox run --otel --herdr opencode run --auto"
alias av="aven tui"

# --- Local secrets (untracked) ---

[ -f "$HOME/.zshrc.secrets" ] && source "$HOME/.zshrc.secrets"

# --- oh-my-posh ---
eval "$(oh-my-posh init zsh --config catppuccin)"

[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"  # This loads nvm bash_completion

# OpenCode OTel tracing
export OPENCODE_ENABLE_TELEMETRY=1
export OPENCODE_OTLP_ENDPOINT=http://localhost:4317
export OPENCODE_OTLP_PROTOCOL=grpc
