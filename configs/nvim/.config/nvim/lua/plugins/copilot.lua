return {
  "zbirenbaum/copilot.lua",
  cmd = "Copilot",
  event = "InsertEnter",
  opts = {
    suggestion = {
      enabled = true,
      auto_trigger = true,
      keymap = {
        accept = "<Tab>",
        next = "<C-]>",
        prev = "<M-[>",
        dismiss = "<C-\\>",
      },
    },
    panel = { enabled = false },
  },
}
