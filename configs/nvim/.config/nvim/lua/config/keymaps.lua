local map = vim.keymap.set

-- window navigation (evil-window style, like spacemacs)
map("n", "<C-h>", "<C-w>h", { desc = "Window left" })
map("n", "<C-j>", "<C-w>j", { desc = "Window down" })
map("n", "<C-k>", "<C-w>k", { desc = "Window up" })
map("n", "<C-l>", "<C-w>l", { desc = "Window right" })

-- window management: SPC w ...
map("n", "<leader>w%", "<cmd>vsplit<CR>", { desc = "Split vertical" })
map("n", "<leader>w\"", "<cmd>split<CR>", { desc = "Split horizontal" })
map("n", "<leader>wd", "<cmd>close<CR>", { desc = "Delete window" })
map("n", "<leader>ww", "<C-w>w", { desc = "Other window" })
map("n", "<leader>w=", "<C-w>=", { desc = "Balance windows" })

-- buffers: SPC b ...
map("n", "<leader>bd", "<cmd>bdelete<CR>", { desc = "Delete buffer" })
map("n", "<leader>bn", "<cmd>bnext<CR>", { desc = "Next buffer" })
map("n", "<leader>bp", "<cmd>bprevious<CR>", { desc = "Previous buffer" })

-- files: SPC f ...
map("n", "<leader>fs", "<cmd>write<CR>", { desc = "Save file" })
map("n", "<leader>fe", "<cmd>edit $MYVIMRC<CR>", { desc = "Edit init.lua" })

-- quit: SPC q ...
map("n", "<leader>qq", "<cmd>qa<CR>", { desc = "Quit all" })

-- toggles: SPC t ...
map("n", "<leader>tn", "<cmd>set relativenumber!<CR>", { desc = "Toggle relative number" })
map("n", "<leader>tw", "<cmd>set wrap!<CR>", { desc = "Toggle wrap" })

-- clear search highlight
map("n", "<Esc>", "<cmd>nohlsearch<CR>", { desc = "Clear search highlight" })

-- better indenting in visual mode
map("v", "<", "<gv", { desc = "Indent left" })
map("v", ">", ">gv", { desc = "Indent right" })

-- move selected lines
map("v", "J", ":m '>+1<CR>gv=gv", { desc = "Move selection down" })
map("v", "K", ":m '<-2<CR>gv=gv", { desc = "Move selection up" })
