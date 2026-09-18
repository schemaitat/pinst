local ensure_installed = {
  "lua", "vim", "vimdoc", "query",
  "bash", "python", "rust", "go",
  "javascript", "typescript", "tsx",
  "json", "yaml", "toml", "markdown", "markdown_inline",
}

return {
  "nvim-treesitter/nvim-treesitter",
  branch = "main",
  build = ":TSUpdate",
  event = { "BufReadPost", "BufNewFile" },
  config = function()
    require("nvim-treesitter").setup()
    require("nvim-treesitter").install(ensure_installed)

    vim.api.nvim_create_autocmd("FileType", {
      group = vim.api.nvim_create_augroup("treesitter-start", { clear = true }),
      callback = function(event)
        local ok = pcall(vim.treesitter.start, event.buf)
        if ok then
          vim.bo[event.buf].indentexpr = "v:lua.require'nvim-treesitter'.indentexpr()"
        end
      end,
    })
  end,
}
