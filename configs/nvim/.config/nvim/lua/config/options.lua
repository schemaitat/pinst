local opt = vim.opt
local g = vim.g

g.mapleader = " "
g.maplocalleader = ","

-- disable unused built-in plugins for faster startup
local disabled_builtins = {
  "gzip", "zip", "zipPlugin", "tar", "tarPlugin",
  "getscript", "getscriptPlugin", "vimball", "vimballPlugin",
  "2html_plugin", "logipat", "rrhelper", "spellfile_plugin",
  "matchit", "matchparen", "netrwPlugin", "netrwSettings",
  "netrwFileHandlers", "tutor",
}
for _, plugin in ipairs(disabled_builtins) do
  g["loaded_" .. plugin] = 1
end

opt.termguicolors = true -- Ghostty is GPU-accelerated, this is cheap now
opt.number = true
opt.relativenumber = true
opt.mouse = "a"
opt.clipboard = "unnamedplus"
opt.splitright = true
opt.splitbelow = true
opt.expandtab = true
opt.tabstop = 2
opt.shiftwidth = 2
opt.smartindent = true
opt.ignorecase = true
opt.smartcase = true
opt.incsearch = true
opt.hlsearch = true
opt.hidden = true
opt.backup = false
opt.writebackup = false
opt.swapfile = false
opt.cmdheight = 1
opt.shortmess:append("c")
opt.signcolumn = "yes"
opt.updatetime = 200
opt.timeoutlen = 400 -- fast which-key popup
opt.undofile = true
opt.undodir = vim.fn.stdpath("state") .. "/undo"
opt.scrolloff = 8
opt.wrap = false
opt.cursorline = true
opt.completeopt = { "menu", "menuone", "noselect" }
opt.pumheight = 12
opt.confirm = true
opt.laststatus = 3
opt.list = true
opt.listchars = { tab = "» ", trail = "·", nbsp = "␣" }

vim.filetype.plugin_on = true
