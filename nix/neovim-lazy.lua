-- Keep the repository's Lua plugin settings, with Nix owning acquisition.
local bundle = vim.json.decode(table.concat(vim.fn.readfile(vim.fn.stdpath("config") .. "/pinst-plugin-paths.json"), "\n"))
-- Lazy takes over package loading before Neovim's normal packloadall pass.
-- Register parser/query dependencies explicitly rather than losing them.
vim.opt.rtp:prepend(bundle.runtime)
local specs = { { import = "plugins" } }
for repo, path in pairs(bundle.plugins) do
  specs[#specs + 1] = { repo, dir = path, build = false }
end
specs[#specs + 1] = { "mason-org/mason.nvim", enabled = false }
specs[#specs + 1] = { "mason-org/mason-lspconfig.nvim", enabled = false }
specs[#specs + 1] = {
  "zbirenbaum/copilot.lua",
  opts = {
    copilot_node_command = bundle.node,
    server = { type = "nodejs", custom_server_filepath = bundle.copilotServer },
  },
}
specs[#specs + 1] = {
  "neovim/nvim-lspconfig",
  lazy = false,
  config = function()
    require("plugins.lsp")[3].config()
    vim.lsp.enable({ "lua_ls", "pyright" })
  end,
}
specs[#specs + 1] = {
  "nvim-treesitter/nvim-treesitter",
  config = function()
    vim.api.nvim_create_autocmd("FileType", {
      group = vim.api.nvim_create_augroup("treesitter-start", { clear = true }),
      callback = function(event)
        if pcall(vim.treesitter.start, event.buf) then
          vim.bo[event.buf].indentexpr = "v:lua.require'nvim-treesitter'.indentexpr()"
        end
      end,
    })
  end,
}
-- Lua matching is deterministic and needs no runtime native-library download.
specs[#specs + 1] = { "saghen/blink.cmp", opts = { fuzzy = { implementation = "lua" } } }
require("lazy").setup({
  spec = specs,
  defaults = { lazy = true },
  install = { missing = false },
  checker = { enabled = false },
  change_detection = { enabled = false },
  rocks = { enabled = false },
  pkg = { enabled = false },
  lockfile = vim.fn.stdpath("state") .. "/lazy-lock.json",
  performance = { rtp = { reset = false } },
})
