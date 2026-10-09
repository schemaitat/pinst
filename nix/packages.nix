{ pkgs }:
with pkgs; [
  curl git unzip zsh direnv oh-my-posh
  ripgrep fd neovim tree-sitter uv python314 nodejs_24 just
  gh delta git-credential-manager
]
