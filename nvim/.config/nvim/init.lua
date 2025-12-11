-- Basic settings
vim.opt.number = true
vim.opt.clipboard = "unnamedplus"
vim.opt.relativenumber = true
vim.opt.expandtab = true
vim.opt.shiftwidth = 2
vim.opt.tabstop = 2
vim.opt.smartindent = true
vim.opt.wrap = false
-- Crash-proof settings for persistent nvim server
vim.opt.swapfile = true
vim.opt.directory = os.getenv("HOME") .. "/.local/share/nvim/swap//"

vim.opt.undofile = true
vim.opt.undodir = os.getenv("HOME") .. "/.local/share/nvim/undo//"

vim.opt.backup = true
vim.opt.backupdir = os.getenv("HOME") .. "/.local/share/nvim/backup//"
vim.opt.writebackup = true

-- Auto-write when switching buffers
vim.opt.autowrite = true
vim.opt.hlsearch = false
vim.opt.incsearch = true
vim.opt.termguicolors = true
vim.opt.scrolloff = 8
vim.opt.signcolumn = "yes"
vim.opt.updatetime = 50

-- Set leader key to space
vim.g.mapleader = " "
vim.g.maplocalleader = " "

-- Allow cursor to wrap to next/previous line
-- Using h,l for left/right wrapping (which map to our j,l keys)
vim.opt.whichwrap:append("h,l")

-- Load keymaps
require("keymaps")

-- Bootstrap lazy.nvim (plugin manager)
local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not vim.loop.fs_stat(lazypath) then
  vim.fn.system({
    "git",
    "clone",
    "--filter=blob:none",
    "https://github.com/folke/lazy.nvim.git",
    "--branch=stable",
    lazypath,
  })
end
vim.opt.rtp:prepend(lazypath)

-- Setup plugins
require("lazy").setup({
  -- Catppuccin theme
  {
    "catppuccin/nvim",
    name = "catppuccin",
    priority = 1000,
    config = function()
      require("catppuccin").setup({
        flavour = "mocha", -- latte, frappe, macchiato, mocha
        transparent_background = true,
        integrations = {
          cmp = true,
          gitsigns = true,
          neotree = true,
          telescope = true,
          treesitter = true,
        },
      })
      vim.cmd.colorscheme "catppuccin"
      -- Force transparency
      vim.api.nvim_set_hl(0, "Normal", { bg = "none" })
      vim.api.nvim_set_hl(0, "NormalFloat", { bg = "none" })
      vim.api.nvim_set_hl(0, "NormalNC", { bg = "none" })
      vim.api.nvim_set_hl(0, "FloatBorder", { bg = "none" })
    end,
  },
  
  -- Codeium AI completion
  {
    "Exafunction/codeium.nvim",
    dependencies = {
      "nvim-lua/plenary.nvim",
      "hrsh7th/nvim-cmp",
    },
    config = function()
      require("codeium").setup({})
    end,
  },
  
  -- Completion framework
  {
    "hrsh7th/nvim-cmp",
    dependencies = {
      "hrsh7th/cmp-buffer",
      "hrsh7th/cmp-path",
      "hrsh7th/cmp-cmdline",
    },
    config = function()
      local cmp = require("cmp")
      
      cmp.setup({
        sources = {
          { name = "codeium" },
          { name = "buffer" },
          { name = "path" },
        },
        mapping = cmp.mapping.preset.insert({
          ["<Tab>"] = cmp.mapping.confirm({ select = true }),
          ["<C-Space>"] = cmp.mapping.complete(),
          ["<C-e>"] = cmp.mapping.abort(),
          ["<C-i>"] = cmp.mapping.select_prev_item(),  -- up in your layout
          ["<C-k>"] = cmp.mapping.select_next_item(),  -- down in your layout
        }),
      })
    end,
  },
  
  -- Telescope (fuzzy finder)
  {
    "nvim-telescope/telescope.nvim",
    branch = "0.1.x",
    dependencies = {
      "nvim-lua/plenary.nvim",
      { "nvim-telescope/telescope-fzf-native.nvim", build = "make" },
    },
    config = function()
      require("telescope").setup({
        defaults = {
          mappings = {
            i = {
              ["<C-k>"] = "move_selection_next",
              ["<C-i>"] = "move_selection_previous",
            },
          },
        },
      })
      require("telescope").load_extension("fzf")
      require("telescope").load_extension("session-lens")
    end,
  },
  
  -- Treesitter (better syntax highlighting)
  {
    "nvim-treesitter/nvim-treesitter",
    build = ":TSUpdate",
    config = function()
      require("nvim-treesitter.configs").setup({
        ensure_installed = { "lua", "vim", "vimdoc", "javascript", "typescript", "python", "php", "html", "css", "json", "yaml", "markdown" },
        auto_install = true,
        highlight = {
          enable = true,
        },
        indent = {
          enable = true,
        },
      })
    end,
  },
  
  -- Which-key (shows keybindings)
  {
    "folke/which-key.nvim",
    event = "VeryLazy",
    config = function()
      require("which-key").setup({
        triggers = {
          -- Disable automatic triggers completely
        },
        defer = function()
          return true -- Always defer (never show automatically)
        end,
      })
      -- Manual trigger with ?
      vim.keymap.set("n", "?", "<cmd>WhichKey<cr>", { desc = "Show keybindings" })
      
      -- Fix Space moving cursor - make it a no-op when pressed alone
      vim.keymap.set("n", "<Space>", "<Nop>", { silent = true })
    end,
  },
  
  -- LSP Support
  {
    "neovim/nvim-lspconfig",
    dependencies = {
      "williamboman/mason.nvim",
      "williamboman/mason-lspconfig.nvim",
      "hrsh7th/cmp-nvim-lsp",
    },
    config = function()
      require("mason").setup()
      require("mason-lspconfig").setup({
        ensure_installed = {
          "ts_ls",           -- TypeScript/JavaScript (updated from tsserver)
          "pyright",         -- Python
          "intelephense",    -- PHP
          "html",
          "cssls",
          "jsonls",
        },
      })

      local capabilities = require("cmp_nvim_lsp").default_capabilities()

      -- Define server configs using new vim.lsp.config API
      local servers = { "ts_ls", "pyright", "intelephense", "html", "cssls", "jsonls" }
      for _, server in ipairs(servers) do
        vim.lsp.config(server, {
          capabilities = capabilities,
        })
        vim.lsp.enable(server)
      end

      -- LSP Keymaps (only when LSP is attached)
      vim.api.nvim_create_autocmd("LspAttach", {
        callback = function(args)
          local opts = { buffer = args.buf }
          vim.keymap.set("n", "gd", vim.lsp.buf.definition, opts)
          vim.keymap.set("n", "gD", vim.lsp.buf.declaration, opts)
          vim.keymap.set("n", "gr", vim.lsp.buf.references, opts)
          vim.keymap.set("n", "gI", vim.lsp.buf.implementation, opts)
          vim.keymap.set("n", "gh", vim.lsp.buf.hover, opts)
          vim.keymap.set("n", "<leader>rn", vim.lsp.buf.rename, opts)
          vim.keymap.set("n", "<leader>ca", vim.lsp.buf.code_action, opts)
          vim.keymap.set("n", "<leader>e", vim.diagnostic.open_float, opts)
          vim.keymap.set("n", "[d", vim.diagnostic.goto_prev, opts)
          vim.keymap.set("n", "]d", vim.diagnostic.goto_next, opts)
        end,
      })
    end,
  },
  
  -- Gitsigns (git integration)
  {
    "lewis6991/gitsigns.nvim",
    config = function()
      require("gitsigns").setup({
        signs = {
          add = { text = "+" },
          change = { text = "~" },
          delete = { text = "_" },
          topdelete = { text = "‾" },
          changedelete = { text = "~" },
        },
      })
    end,
  },
  
  -- Mini.nvim (utilities)
  {
    "echasnovski/mini.nvim",
    version = false,
    config = function()
      -- Commenting with gcc/gc
      require("mini.comment").setup()
      
      -- Surround text objects (add/delete/change surroundings)
      require("mini.surround").setup()
      
      -- Auto pairs for brackets
      require("mini.pairs").setup()
      
      -- Better text objects
      require("mini.ai").setup({
       mappings = {
         around = 'A',
         inside = 'I',
         around_next = 'an',
         inside_next = 'in',
         around_last = 'al',
         inside_last = 'il',
      },})
    end,
  },
  
  -- Lualine (status bar)
  {
    "nvim-lualine/lualine.nvim",
    dependencies = { "nvim-tree/nvim-web-devicons" },
    config = function()
      require("lualine").setup({
        options = {
          theme = "catppuccin",
          component_separators = { left = "", right = "" },
          section_separators = { left = "", right = "" },
        },
      })
    end,
  },
  
  -- snacks.nvim (QoL utilities + image preview)
  {
    "folke/snacks.nvim",
    priority = 1000,
    lazy = false,
    opts = {
      image = { enabled = true },
    },
  },

  -- Smooth scrolling (keyboard only)
  {
    "karb94/neoscroll.nvim",
    config = function()
      require("neoscroll").setup({
        mappings = { "<C-u>", "<C-d>", "<C-b>", "<C-f>", "zt", "zz", "zb" },
      })
    end,
  },

  -- neo-tree (file explorer)
  {
    "nvim-neo-tree/neo-tree.nvim",
    branch = "v3.x",
    dependencies = {
      "nvim-lua/plenary.nvim",
      "nvim-tree/nvim-web-devicons",
      "MunifTanjim/nui.nvim",
    },
    config = function()
      require("neo-tree").setup({
        enable_git_status = false,
        close_if_last_window = false,
        filesystem = {
          follow_current_file = { enabled = true },
          use_libuv_file_watcher = true,
        },
        window = {
          position = "right",
          width = 30,
        },
        buffers = {
          follow_current_file = { enabled = true },
        },
        default_component_configs = {
          indent = {
            with_expanders = true,
          },
        },
      })
    end,
  },

  -- Claude Code integration
  {
    "coder/claudecode.nvim",
    config = true,
  },
  
  -- Bufferline (tabs for open files)
  {
    "akinsho/bufferline.nvim",
    version = "*",
    dependencies = { "nvim-tree/nvim-web-devicons" },
    config = function()
      require("bufferline").setup({
        options = {
          theme = "catppuccin",
          offsets = {
            {
              filetype = "neo-tree",
              text = "File Explorer",
              highlight = "Directory",
              text_align = "center",
              separator = true,
            },
          },
          diagnostics = "nvim_lsp",
          show_buffer_close_icons = false,
          show_close_icon = false,
          separator_style = "thin",
          always_show_bufferline = false,
        },
      })
    end,
  },

  -- Auto save sessions
  {
    "rmagatti/auto-session",
    config = function()
      require("auto-session").setup({
        log_level = "error",
        auto_restore_enabled = false,
      })
    end,
  },

  -- CodeCompanion
  {
    "olimorris/codecompanion.nvim",
    version = "v17.33.0",
    dependencies = {
      "nvim-lua/plenary.nvim",
    },
    opts = {
      adapters = {
        acp = {
          claude_code = function()
            return require("codecompanion.adapters").extend("claude_code", {
              env = {
                CLAUDE_CODE_OAUTH_TOKEN = os.getenv("CLAUDE_CODE_OAUTH_TOKEN"),
              },
            })
          end,
        },
      },
      strategies = {
        chat = {
          adapter = "claude_code",
        },
      },
    },
  },
})

-- Auto-save on focus lost (popup close, etc)
vim.api.nvim_create_autocmd('FocusLost', {
  pattern = '*',
  command = 'silent! wa'
})

-- Auto-save before leaving buffer
vim.api.nvim_create_autocmd('BufLeave', {
  pattern = '*',
  callback = function()
    if vim.bo.modified and vim.bo.buftype == '' then
      vim.cmd('silent! write')
    end
  end
})

-- Neo-tree keymaps
vim.keymap.set("n", "<leader>e", ":Neotree toggle<CR>", { desc = "Toggle file explorer" })
vim.keymap.set("n", "<leader>o", ":Neotree focus<CR>", { desc = "Focus file explorer" })
vim.keymap.set("n", "<leader>b", ":Neotree buffers<CR>", { desc = "Show open buffers" })
vim.keymap.set("n", "<leader>gs", ":Neotree git_status<CR>", { desc = "Git status" })

-- Enable wrap for prose filetypes
vim.api.nvim_create_autocmd("FileType", {
  pattern = { "markdown", "text" },
  callback = function()
    vim.opt_local.wrap = true
  end,
})

-- Prompt-editor mode (Claude Code)
if os.getenv("CLAUDE_PROMPT_EDITOR") then
  vim.opt.wrap = true
  -- Auto-save on quit
  vim.api.nvim_create_autocmd("QuitPre", {
    pattern = "*",
    callback = function()
      if vim.bo.modified and vim.bo.buftype == "" then
        vim.cmd("silent! write")
      end
    end,
  })

  -- Ctrl+G to quit AND signal auto-submit
  vim.keymap.set("n", "<C-g>", function()
    io.open("/tmp/claude-submit", "w"):close()
    vim.cmd("qa!")
  end, { silent = true })
  vim.keymap.set("i", "<C-g>", function()
    vim.cmd("stopinsert")
    io.open("/tmp/claude-submit", "w"):close()
    vim.cmd("qa!")
  end, { silent = true })
end
