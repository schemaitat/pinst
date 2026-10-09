{ pkgs, ... }:
let
  # The stable package's v2.0.4 archive failed its hash check. Use the exact
  # revision already recorded in this repo's lazy-lock.json instead of
  # accepting changed bytes behind that tag.
  copilot = pkgs.vimUtils.buildVimPlugin {
    pname = "copilot.lua";
    version = "locked-b2b899f";
    src = pkgs.fetchFromGitHub {
      owner = "zbirenbaum";
      repo = "copilot.lua";
      rev = "b2b899f93544047ac94d452ecd5af1e63a94377a";
      hash = "sha256-RYi+Ofn+kqXkw8z2f1avH62MMLqVyIIDJaYBCMt/fBw=";
    };
  };
  treesitter = pkgs.vimPlugins.nvim-treesitter.withPlugins (p: with p; [
    lua vim vimdoc query bash python rust go javascript typescript tsx
    json yaml toml markdown markdown_inline
  ]);
  collect = ps: pkgs.lib.unique (ps ++ pkgs.lib.concatMap (p: collect (p.dependencies or [ ])) ps);
  parserRuntime = pkgs.symlinkJoin {
    name = "pinst-treesitter-runtime";
    paths = collect treesitter.dependencies;
  };
  # Every plugin from the native Lua specs resolves to a store directory.
  # Builds (fzf, parsers, completion) happen in Nix, never on first startup.
  plugins = with pkgs.vimPlugins; {
    "folke/tokyonight.nvim" = tokyonight-nvim;
    "folke/which-key.nvim" = which-key-nvim;
    "zbirenbaum/copilot.lua" = copilot;
    "numToStr/Comment.nvim" = comment-nvim;
    "kylechui/nvim-surround" = nvim-surround;
    "windwp/nvim-autopairs" = nvim-autopairs;
    "lewis6991/gitsigns.nvim" = gitsigns-nvim;
    "kdheepak/lazygit.nvim" = lazygit-nvim;
    "nvim-lua/plenary.nvim" = plenary-nvim;
    "neovim/nvim-lspconfig" = nvim-lspconfig;
    "nvim-lualine/lualine.nvim" = lualine-nvim;
    "MeanderingProgrammer/render-markdown.nvim" = render-markdown-nvim;
    "nvim-telescope/telescope.nvim" = telescope-nvim;
    "nvim-telescope/telescope-fzf-native.nvim" = telescope-fzf-native-nvim;
    "nvim-tree/nvim-tree.lua" = nvim-tree-lua;
    "nvim-tree/nvim-web-devicons" = nvim-web-devicons;
    "akinsho/toggleterm.nvim" = toggleterm-nvim;
    "saghen/blink.cmp" = blink-cmp;
    "nvim-treesitter/nvim-treesitter" = treesitter;
  };
  paths = pkgs.writeText "pinst-plugin-paths.json" (builtins.toJSON {
    plugins = builtins.mapAttrs (_: p: "${p}") plugins;
    copilotServer = "${pkgs.copilot-language-server}/share/copilot-language-server/main.js";
    node = "${pkgs.nodejs_24}/bin/node";
    runtime = "${parserRuntime}";
  });
  configLua = pkgs.runCommand "pinst-nvim-lua" { } ''
    mkdir -p "$out"
    cp -R ${../configs/nvim/.config/nvim/lua}/. "$out/"
    chmod -R u+w "$out"
    cp ${./neovim-lazy.lua} "$out/config/lazy.lua"
  '';
in {
  programs.neovim = {
    enable = true;
    defaultEditor = true;
    vimAlias = true;
    withNodeJs = false;
    withPython3 = false;
    plugins = [ pkgs.vimPlugins.lazy-nvim treesitter ];
    extraPackages = with pkgs; [ lua-language-server pyright nodejs_24 ripgrep fd lazygit ];
    initLua = builtins.readFile ../configs/nvim/.config/nvim/init.lua;
  };
  xdg.configFile."nvim/lua" = { source = configLua; recursive = true; };
  xdg.configFile."nvim/pinst-plugin-paths.json".source = paths;
}
