return {
  "folke/which-key.nvim",
  event = "VeryLazy",
  opts = {
    preset = "modern",
    delay = 250,
    spec = {
      { "<leader>f", group = "file" },
      { "<leader>b", group = "buffer" },
      { "<leader>w", group = "window" },
      { "<leader>p", group = "project" },
      { "<leader>s", group = "search" },
      { "<leader>g", group = "git" },
      { "<leader>t", group = "toggle" },
      { "<leader>q", group = "quit" },
    },
  },
}
