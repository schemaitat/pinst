return {
  "nvim-telescope/telescope.nvim",
  cmd = "Telescope",
  dependencies = {
    "nvim-lua/plenary.nvim",
    { "nvim-telescope/telescope-fzf-native.nvim", build = "make" },
  },
  keys = {
    { "<leader>ff", "<cmd>Telescope find_files<CR>", desc = "Find files" },
    { "<leader>pf", "<cmd>Telescope find_files<CR>", desc = "Find files in project" },
    { "<leader>fr", "<cmd>Telescope oldfiles<CR>", desc = "Recent files" },
    { "<leader>bb", "<cmd>Telescope buffers<CR>", desc = "Find buffer" },
    { "<leader>ss", "<cmd>Telescope live_grep<CR>", desc = "Search in project" },
    { "<leader>/", "<cmd>Telescope live_grep<CR>", desc = "Search in project" },
    { "<leader><leader>", "<cmd>Telescope commands<CR>", desc = "Command palette" },
    { "<leader>sb", "<cmd>Telescope current_buffer_fuzzy_find<CR>", desc = "Search in buffer" },
    { "<leader>sh", "<cmd>Telescope help_tags<CR>", desc = "Search help" },
    { "<leader>sd", "<cmd>Telescope lsp_document_symbols<CR>", desc = "Document symbols" },
    { "<leader>sw", "<cmd>Telescope lsp_dynamic_workspace_symbols<CR>", desc = "Workspace symbols" },
  },
  opts = {
    defaults = {
      layout_strategy = "horizontal",
      sorting_strategy = "ascending",
      layout_config = { prompt_position = "top" },
      file_ignore_patterns = {
        "%.git/",
        "node_modules/",
        "__pycache__/",
        "%.venv/",
        "venv/",
        "dist/",
        "build/",
        "target/",
        "%.mypy_cache/",
        "%.pytest_cache/",
        "%.ruff_cache/",
      },
    },
    pickers = {
      find_files = {
        hidden = true,
        find_command = { "fd", "--type", "f", "--hidden", "--exclude", ".git" },
      },
    },
  },
  config = function(_, opts)
    local telescope = require("telescope")
    telescope.setup(opts)
    pcall(telescope.load_extension, "fzf")
  end,
}
