return {
  "nvim-tree/nvim-tree.lua",
  dependencies = { "nvim-tree/nvim-web-devicons" },
  cmd = { "NvimTreeToggle", "NvimTreeFocus" },
  keys = {
    { "<leader>tt", "<cmd>NvimTreeToggle<CR>", desc = "Toggle file tree" },
    { "<leader>fT", "<cmd>NvimTreeFocus<CR>", desc = "Focus file tree" },
  },
  opts = {
    disable_netrw = true,
    hijack_netrw = true,
    respect_buf_cwd = true,
    sync_root_with_cwd = true,
    view = { width = 32 },
    renderer = {
      group_empty = true,
      icons = { show = { git = true, folder = true, file = true, folder_arrow = true } },
    },
    filters = { dotfiles = false },
    git = { enable = true },
    actions = { open_file = { quit_on_open = false } },
  },
}
