return {
  "akinsho/toggleterm.nvim",
  version = "*",
  keys = {
    { "<D-m>", "<cmd>ToggleTerm direction=float<CR>", mode = { "n", "i", "v" }, desc = "Toggle terminal" },
    { "<leader>tf", "<cmd>ToggleTerm direction=float<CR>", mode = "n", desc = "Toggle float terminal" },
    { "<leader>ts", "<cmd>ToggleTerm direction=horizontal<CR>", mode = "n", desc = "Toggle split terminal" },
  },
  opts = {
    open_mapping = nil, -- we drive it entirely via <D-m>
    direction = "float",
    float_opts = { border = "curved" },
    start_in_insert = true,
    shade_terminals = true,
    size = 15,
  },
  config = function(_, opts)
    require("toggleterm").setup(opts)
    -- Cmd+M must also work from *inside* the terminal buffer to close it
    vim.api.nvim_create_autocmd("TermOpen", {
      pattern = "term://*toggleterm#*",
      callback = function(event)
        vim.keymap.set("t", "<D-m>", "<cmd>ToggleTerm<CR>", { buffer = event.buf, desc = "Toggle terminal" })
        vim.keymap.set("t", "<Esc>", [[<C-\><C-n>]], { buffer = event.buf, desc = "Terminal normal mode" })
        vim.keymap.set(
          "t",
          "<Space>tf",
          [[<C-\><C-n><cmd>ToggleTerm direction=float<CR>]],
          { buffer = event.buf, desc = "Toggle float terminal" }
        )
        vim.keymap.set(
          "t",
          "<Space>ts",
          [[<C-\><C-n><cmd>ToggleTerm direction=horizontal<CR>]],
          { buffer = event.buf, desc = "Toggle split terminal" }
        )
      end,
    })
  end,
}
