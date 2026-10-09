{ config, lib, pkgs, ... }:
let
  external = builtins.fromJSON (builtins.readFile ./external-tools.json);
in {
  home.stateVersion = "26.05";
  home.packages = import ./packages.nix { inherit pkgs; };
  xdg.enable = true;
  targets.genericLinux.enable = pkgs.stdenv.hostPlatform.isLinux;

  programs.zsh = {
    enable = true;
    dotDir = config.home.homeDirectory;
    autosuggestion.enable = true;
    oh-my-zsh = {
      enable = true;
      plugins = [ "git" "z" "sudo" "colored-man-pages" ];
    };
    initContent = ''
      # The framework and plugins are immutable; updates happen through flake.lock.
      zstyle ':omz:update' mode disabled
      HYPHEN_INSENSITIVE=true
      COMPLETION_WAITING_DOTS=true
      zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'
      zstyle ':completion:*' menu select
      [ ! -f "$HOME/.zshrc.secrets" ] || source "$HOME/.zshrc.secrets"
      [ ! -f "$HOME/.config/pinst/local.zsh" ] || source "$HOME/.config/pinst/local.zsh"
    '';
    envExtra = ''
      export PATH="$HOME/.nix-profile/bin:$HOME/.local/state/nix/profiles/profile/bin:$HOME/.local/bin:$HOME/.opencode/bin:$PATH"
    '';
    shellAliases = { v = "nvim"; ll = "ls -alh"; oc = "opencode"; av = "aven tui"; };
  };
  programs.direnv.enable = true;
  programs.oh-my-posh = {
    enable = true;
    useTheme = "catppuccin";
  };
  # Identity is a runtime include, so a user's identity/credentials are never
  # copied into the store by evaluating this flake.
  programs.git = {
    enable = true;
    includes = [ { path = "~/.config/pinst/git-identity"; } ];
    settings = {
      core = { editor = "nvim"; pager = "delta"; };
      init.defaultBranch = "main";
      interactive.diffFilter = "delta --color-only";
      delta = { navigate = true; line-numbers = true; side-by-side = true; };
      merge.conflictstyle = "diff3";
      diff.colorMoved = "default";
      credential."https://github.com".helper = [ "" "!${pkgs.gh}/bin/gh auth git-credential" ];
      credential."https://gist.github.com".helper = [ "" "!${pkgs.gh}/bin/gh auth git-credential" ];
    };
  };
  home.file.".gitconfig".text = "# Managed Git settings are in ~/.config/git/config.\n";
  xdg.configFile."herdr/config.toml".source = ../configs/herdr/.config/herdr/config.toml;
  # Replaced by the offline editor package in the readiness phase.
  xdg.configFile."nvim" = { source = ../configs/nvim/.config/nvim; recursive = true; };
  xdg.configFile."pinst/environment.json".text = builtins.toJSON {
    schema_version = 1;
    owner = "home-manager";
    home = config.home.homeDirectory;
    inherit external;
  };
}
