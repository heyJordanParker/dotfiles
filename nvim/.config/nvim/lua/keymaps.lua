local keymap = vim.keymap.set
local opts = { noremap = true, silent = true }

-- Core movement remapping: j=left, k=down, l=right, i=up
keymap('n', 'j', 'h', opts)
keymap('n', 'k', 'j', opts)
keymap('n', 'i', 'k', opts)
-- l stays as right

keymap('v', 'j', 'h', opts)
keymap('v', 'k', 'j', opts)
keymap('v', 'i', 'k', opts)

keymap('x', 'j', 'h', opts)
keymap('x', 'k', 'j', opts)
keymap('x', 'i', 'k', opts)

-- Operator-pending mode (for commands like dj, yk, etc.)
keymap('o', 'j', 'h', opts)
keymap('o', 'k', 'j', opts)
keymap('o', 'i', 'k', opts)

-- Remap text objects to use I (inner) and A (around) instead of i/a
-- This allows 'i' to be used for upward movement
for _, char in ipairs({'"', "'", '`', '(', ')', '{', '}', '[', ']', '<', '>'}) do
  keymap('v', 'I' .. char, 'i' .. char, opts)  -- Inner text object
  keymap('v', 'A' .. char, 'a' .. char, opts)  -- Around text object
  keymap('o', 'I' .. char, 'i' .. char, opts)  -- For operations like dI", cI(
  keymap('o', 'A' .. char, 'a' .. char, opts)  -- For operations like dA", cA(
end

-- Word, WORD, sentence, paragraph text objects
for _, obj in ipairs({'w', 'W', 's', 'p', 'b', 'B', 't'}) do
  keymap('v', 'I' .. obj, 'i' .. obj, opts)
  keymap('v', 'A' .. obj, 'a' .. obj, opts)
  keymap('o', 'I' .. obj, 'i' .. obj, opts)
  keymap('o', 'A' .. obj, 'a' .. obj, opts)
end

-- Escape with jj
keymap('i', 'jj', '<Esc>', opts)

-- Insert mode: Enter key
keymap('n', '<CR>', 'a', opts)
keymap('n', '<S-CR>', 'i', opts)
keymap('n', 'I', 'gk', opts)  -- Capital I moves up (visual line up)
keymap('v', '<CR>', '<Esc>i', opts)  -- Exit visual and enter insert
keymap('x', '<CR>', '<Esc>i', opts)  -- Exit visual block and enter insert

-- Alt + movement for insert entry (directional mnemonics)
keymap('n', '<M-j>', 'i', opts)  -- insert before cursor (left)
keymap('n', '<M-l>', 'a', opts)  -- append after cursor (right)
keymap('n', '<M-k>', 'o', opts)  -- new line below (down)
keymap('n', '<M-i>', 'O', opts)  -- new line above (up)
keymap('n', '<M-u>', 'I', opts)  -- insert at line start
keymap('n', '<M-o>', 'A', opts)  -- append at line end

-- Select all
keymap('n', 'A', 'ggVG', opts)  -- Select entire file

-- Line navigation: u=beginning, o=end
keymap('n', 'u', '^', opts)
keymap('n', 'o', '$', opts)
keymap('v', 'u', '^', opts)
keymap('v', 'o', '$', opts)
keymap('x', 'u', '^', opts)
keymap('x', 'o', '$', opts)
keymap('o', 'u', '^', opts)
keymap('o', 'o', '$', opts)

-- Word navigation: h=backward, ;=forward
keymap('n', 'h', 'b', opts)
keymap('n', ';', 'w', opts)
keymap('v', 'h', 'b', opts)
keymap('v', ';', 'w', opts)
keymap('x', 'h', 'b', opts)
keymap('x', ';', 'w', opts)
keymap('o', 'h', 'b', opts)
keymap('o', ';', 'w', opts)

-- Editing commands (n/N restored to vim default for search navigation)
keymap('n', 'z', 'u', opts)  -- undo
keymap('n', 'Z', '<C-r>', opts)  -- redo

-- File navigation: U=start, O=end
keymap('n', 'U', 'gg', opts)  -- go to start of file
keymap('n', 'O', 'G', opts)   -- go to end of file
keymap('v', 'U', 'gg', opts)
keymap('v', 'O', 'G', opts)
keymap('x', 'U', 'gg', opts)
keymap('x', 'O', 'G', opts)

-- Search navigation: n/N restored to vim default, [/] freed

-- Motions & repeat: .=edit repeat (default), ,=f/t repeat, <=f/t reverse, >=Ex repeat
keymap('n', ',', ';', opts)  -- f/t repeat (was ;)
keymap('n', '<', ',', opts)  -- f/t reverse (was ,)
keymap('n', '>', '@:', opts) -- repeat Ex command
keymap('v', ',', ';', opts)
keymap('v', '<', ',', opts)
keymap('x', ',', ';', opts)
keymap('x', '<', ',', opts)
keymap('o', ',', ';', opts)
keymap('o', '<', ',', opts)

-- Window navigation with Ctrl+w (keeping for compatibility)
keymap('n', '<C-w>j', '<C-w>h', opts)  -- left window
keymap('n', '<C-w>k', '<C-w>j', opts)  -- down window
keymap('n', '<C-w>i', '<C-w>k', opts)  -- up window
-- <C-w>l stays as right window

-- Also remap the uppercase versions for window movement
keymap('n', '<C-w>J', '<C-w>H', opts)
keymap('n', '<C-w>K', '<C-w>J', opts)
keymap('n', '<C-w>I', '<C-w>K', opts)

-- Window navigation with leader (easier!)
keymap('n', '<leader>j', '<C-w>h', opts)  -- move to left window
keymap('n', '<leader>k', '<C-w>j', opts)  -- move to window below
keymap('n', '<leader>l', '<C-w>l', opts)  -- move to right window
keymap('n', '<leader>i', '<C-w>k', opts)  -- move to window above

-- Window splitting with leader + capitals (and move to new pane)
keymap('n', '<leader>J', '<C-w>v<C-w>h', opts)  -- split left and move there
keymap('n', '<leader>K', '<C-w>s<C-w>j', opts)  -- split below and move there
keymap('n', '<leader>L', '<C-w>v', opts)         -- split right and move there
keymap('n', '<leader>I', '<C-w>s', opts)         -- split above and move there

-- Fix any remaining h references that might be needed
keymap('n', 'H', 'B', opts)  -- WORD backward (since h is word backward)
-- Removed : remapping to keep command mode working
keymap('v', 'H', 'B', opts)
keymap('x', 'H', 'B', opts)
keymap('o', 'H', 'B', opts)

-- Leader key mappings (Space + key)
keymap('n', '<leader>s', ':w<CR>', opts)  -- Space+s to save
keymap('n', '<leader>sq', ':wqa<CR>', opts)  -- Space+sq to save all and quit
keymap('n', '<leader>sw', ':w<CR>:close<CR>', opts)  -- Space+sw to save and close window
keymap('n', '<leader>w', ':q!<CR>', opts)  -- Space+w to close window (or quit if last)
keymap('n', '<leader>W', ':w<CR>:close<CR>', opts)  -- Space+W to save and close window
keymap('n', '<leader>q', ':qa!<CR>', opts)  -- Space+q to force quit all (without saving)
keymap('n', '<leader>C', ':source $MYVIMRC<CR>', opts)  -- Space+C to reload config
-- Q unmapped (does nothing)
-- W unmapped (does nothing)

-- Leader + copy/paste with system clipboard
keymap('n', '<leader>c', '"+y', opts)  -- Space+c to copy to system clipboard
keymap('v', '<leader>c', '"+y', opts)  -- Space+c in visual mode
keymap('n', '<leader>v', '"+p', opts)  -- Space+v to paste from system clipboard
keymap('n', '<leader>V', '"+P', opts)  -- Space+V to paste before cursor

-- Telescope keybindings
keymap('n', '<leader>f', '<cmd>Telescope current_buffer_fuzzy_find<cr>', { desc = 'Find in current file', silent = true })
keymap('n', '<leader>G', '<cmd>Telescope live_grep<cr>',                 { desc = 'Grep in all files',    silent = true })
keymap('n', '<leader>t', '<cmd>Telescope find_files<cr>',                { desc = 'Find files by name',   silent = true })
keymap('n', '<leader>b', '<cmd>Telescope buffers<cr>',                   { desc = 'Find buffers',         silent = true })
keymap('n', '<leader>r', '<cmd>Telescope oldfiles<cr>',                  { desc = 'Recent files',         silent = true })
keymap('n', '<leader>fh','<cmd>Telescope help_tags<cr>',                 { desc = 'Find help',            silent = true })
keymap('n', '<leader>gf','<cmd>Telescope git_files<cr>',                 { desc = 'Git files',            silent = true })
keymap('n', '<leader>gc','<cmd>Telescope git_commits<cr>',               { desc = 'Git commits',          silent = true })
keymap('n', '<leader>gb','<cmd>Telescope git_branches<cr>',              { desc = 'Git branches',         silent = true })

-- NvimTree keybindings
keymap('n', '<leader>e', '<cmd>NvimTreeToggle<cr>', { desc = 'Toggle file explorer' })
keymap('n', '<leader>E', '<cmd>NvimTreeFindFile<cr>', { desc = 'Find current file in explorer' })

-- Indent/unindent (all modes)
keymap('n', '<Tab>', '>>', { desc = 'Indent line' })
keymap('n', '<S-Tab>', '<<', { desc = 'Unindent line' })
keymap('v', '<Tab>', '>gv', { desc = 'Indent selection' })
keymap('v', '<S-Tab>', '<gv', { desc = 'Unindent selection' })
keymap('i', '<Tab>', '<C-t>', { desc = 'Indent' })
keymap('i', '<S-Tab>', '<C-d>', { desc = 'Unindent' })

-- Buffer navigation (leader + Tab)
keymap('n', '<leader><Tab>', '<cmd>BufferLineCycleNext<cr>', { desc = 'Next buffer' })
keymap('n', '<leader><S-Tab>', '<cmd>BufferLineCyclePrev<cr>', { desc = 'Previous buffer' })
keymap('n', '<leader>x', '<cmd>bd<cr>', { desc = 'Close current buffer' })
keymap('n', '<leader>X', '<cmd>BufferLineCloseOthers<cr>', { desc = 'Close other buffers' })

