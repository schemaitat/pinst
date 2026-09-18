return {
  {
    "mason-org/mason.nvim",
    cmd = "Mason",
    opts = {},
  },
  {
    "mason-org/mason-lspconfig.nvim",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = {
      "mason-org/mason.nvim",
      "neovim/nvim-lspconfig",
    },
    opts = {
      ensure_installed = { "lua_ls" },
      automatic_enable = true,
    },
  },
  {
    "neovim/nvim-lspconfig",
    lazy = true,
    config = function()
      vim.lsp.config("lua_ls", {
        settings = {
          Lua = {
            diagnostics = { globals = { "vim" } },
            workspace = { checkThirdParty = false },
          },
        },
      })

      -- Point pyright at the project's virtualenv (e.g. uv-managed .venv) so
      -- it resolves third-party imports instead of falling back to system python.
      -- client.settings is snapshotted before before_init runs, so the path
      -- must be pushed via on_init + a manual didChangeConfiguration notify.
      vim.lsp.config("pyright", {
        on_init = function(client)
          local root = client.root_dir
          if not root then
            return
          end
          for _, venv in ipairs({ ".venv", "venv" }) do
            local python = root .. "/" .. venv .. "/bin/python"
            if vim.uv.fs_stat(python) then
              client.settings = vim.tbl_deep_extend("force", client.settings, {
                python = { pythonPath = python },
              })
              client:notify("workspace/didChangeConfiguration", { settings = client.settings })
              break
            end
          end
        end,
      })

      vim.diagnostic.config({
        virtual_text = true,
        severity_sort = true,
        float = { border = "rounded" },
      })

      vim.api.nvim_create_autocmd("LspAttach", {
        group = vim.api.nvim_create_augroup("lsp-attach", { clear = true }),
        callback = function(event)
          local opts = { buffer = event.buf }
          vim.keymap.set("n", "gd", vim.lsp.buf.definition, vim.tbl_extend("force", opts, { desc = "Go to definition" }))
          vim.keymap.set("n", "gr", vim.lsp.buf.references, vim.tbl_extend("force", opts, { desc = "Go to references" }))
          vim.keymap.set("n", "K", vim.lsp.buf.hover, vim.tbl_extend("force", opts, { desc = "Hover docs" }))
          vim.keymap.set("n", "<leader>ca", vim.lsp.buf.code_action, vim.tbl_extend("force", opts, { desc = "Code action" }))
          vim.keymap.set("n", "<leader>rn", vim.lsp.buf.rename, vim.tbl_extend("force", opts, { desc = "Rename symbol" }))
          vim.keymap.set("n", "<leader>fd", vim.diagnostic.open_float, vim.tbl_extend("force", opts, { desc = "Line diagnostics" }))
        end,
      })
    end,
  },
}
