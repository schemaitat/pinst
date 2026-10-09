local ok, err = xpcall(function()
  assert(vim.v.errmsg == "", "startup error: " .. vim.v.errmsg)
  local plugins = require("lazy.core.config").plugins
  assert(plugins["telescope.nvim"] and plugins["nvim-lspconfig"])
  for name, plugin in pairs(plugins) do
    local resolved = vim.uv.fs_realpath(plugin.dir)
    assert(resolved and resolved:match("^/nix/store/"), name .. " is not store-backed: " .. plugin.dir)
    assert(vim.fn.isdirectory(plugin.dir) == 1, name .. " is missing")
  end
  assert(not plugins["mason.nvim"] and not plugins["mason-lspconfig.nvim"])
  require("telescope").load_extension("fzf")
  for _, language in ipairs({ "lua", "vim", "vimdoc", "query", "bash", "python", "rust", "go",
    "javascript", "typescript", "tsx", "json", "yaml", "toml", "markdown", "markdown_inline" }) do
    assert(vim.treesitter.language.add(language), "missing parser: " .. language)
  end
  local parser = vim.treesitter.get_string_parser("local x = 1", "lua")
  assert(not parser:parse()[1]:root():has_error())
  assert(vim.fn.executable("lua-language-server") == 1)
  assert(vim.fn.executable("pyright-langserver") == 1)
  local bundle = vim.json.decode(table.concat(vim.fn.readfile(vim.fn.stdpath("config") .. "/pinst-plugin-paths.json"), "\n"))
  assert(vim.fn.filereadable(bundle.copilotServer) == 1, "Copilot server must not need downloading")
  local project = vim.env.HOME .. "/smoke-project"
  vim.fn.mkdir(project, "p")
  vim.fn.writefile({ "{}" }, project .. "/.luarc.json")
  vim.fn.writefile({ "[project]", 'name = "smoke"', 'version = "0.0.0"' }, project .. "/pyproject.toml")
  for _, spec in ipairs({ { "example.lua", "local x = 1", "lua_ls" }, { "example.py", "x = 1", "pyright" } }) do
    vim.fn.writefile({ spec[2] }, project .. "/" .. spec[1])
    vim.cmd.edit(project .. "/" .. spec[1])
    assert(vim.wait(15000, function()
      for _, client in ipairs(vim.lsp.get_clients({ name = spec[3], bufnr = 0 })) do
        if client.initialized then return true end
      end
      return false
    end, 100), spec[3] .. " did not initialize")
  end
  assert(vim.v.errmsg == "", "editor error: " .. vim.v.errmsg)
end, debug.traceback)
if not ok then
  io.stderr:write(err .. "\n")
  vim.cmd("cquit 1")
else
  print("Neovim readiness: store plugins, native fzf, parsers and LSP startup passed")
  vim.cmd("qa!")
end
