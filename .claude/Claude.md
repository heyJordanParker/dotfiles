# Dotfiles
v1.4 | Updated: 2026-04-26

## Why

Reproducible macOS environment setup and Claude Code plugin distribution from a single repository.

## What

GNU Stow-managed dotfiles. Each subdirectory is a package. Also serves as a Claude Code plugin marketplace.

### Requirements

- Every config must be stow-manageable — mirror target directory structure inside each package
- Plugin marketplace must distribute skills, hooks, and commands without leaking local config
- Must restow after editing: `stow -R <package>`

### Boundaries

- Never commit `settings.json` or `Claude.md` to plugin distribution — these are local only
- Never put rules in plugin manifest — not supported by plugin schema
- Never break stow symlink structure — packages mirror their target paths exactly

## Architecture

Each top-level directory is a stow package mirroring its target path. `claude/` doubles as the plugin marketplace root.

!`ls -d */ | grep -v '^\.'`

```
dotfiles/
├── .claude-plugin/
│   └── marketplace.json        # marketplace catalog
├── atuin/                      # shell history
├── bat/                        # cat replacement
├── bin/                        # custom scripts → ~/.local/bin/
├── borders/                    # window borders
├── btop/                       # system monitor
├── bun/                        # JS runtime
├── claude/.claude/             # Claude Code config → ~/.claude/
│   ├── agents/                 #   custom subagents (LOCAL ONLY)
│   ├── skills/                 #   plugin: auto-discovered
│   ├── commands/               #   plugin: auto-discovered
│   ├── hooks/                  #   plugin: hooks.json wiring
│   ├── settings.json           #   LOCAL ONLY — not distributed
│   ├── keybindings.json        #   LOCAL ONLY — per-user shortcuts
│   └── Claude.md               #   LOCAL ONLY — not distributed
├── codex/                      # OpenAI Codex
├── delta/                      # git diff pager
├── ghostty/                    # terminal emulator
├── git/                        # git config + hooks
├── hyprspace/                  # window management
├── karabiner/                  # keyboard config (HRM + layers)
├── lazygit/                    # git TUI
├── npm/                        # npm config
├── nvim/                       # neovim
├── opencode/                   # OpenAI CLI
├── ssh/                        # SSH config
├── starship/                   # shell prompt
├── superfile/                  # file manager TUI
├── tmux/                       # terminal multiplexer
├── zed/                        # code editor
├── zsh/                        # shell config
├── Brewfile                    # homebrew dependencies
├── Claude.md                   # root docs (keybindings, ~/Developer)
└── setup.sh                    # bootstrap script
```

## Workflow

### After Editing

```bash
stow -R <package>   # e.g., stow -R zsh
```

### Adding New Packages

1. Create package directory: `mkdir <package>`
2. Mirror the target structure inside:
   ```
   <package>/
   └── .config/
       └── <app>/
           └── config.toml
   ```
3. Stow it: `stow <package>`

### Installing CLI Tools

1. Add to `Brewfile` (appropriate section)
2. If config needed: create package dir, add config files
3. If wrapper needed (secrets, env vars): add to `bin/.local/bin/`
4. Stow any new/modified packages: `stow -R <package>`

### Python Tools (pipx)

```bash
pipx install <package>
```

## How

### Plugin Marketplace

Users install with:
```
/plugin marketplace add heyJordanParker/dotfiles
/plugin install talents@talent-tree
```

**What gets distributed:** Skills (`skills/`), Commands (`commands/*.md`), Hooks (`hooks/hooks.json` with `${CLAUDE_PLUGIN_ROOT}` paths)

**What does NOT get distributed:** Rules, Settings, Agents, `settings.json`, `Claude.md`, tmux hooks

### Dual Hooks Setup

Hook **scripts/content** are shared (single files in `hooks/`). Hook **wiring** exists in two places:
- `settings.json` — local use via stow (includes tmux hooks)
- `hooks/hooks.json` — plugin consumers (non-tmux hooks only, uses `${CLAUDE_PLUGIN_ROOT}`)

When adding/changing a non-tmux hook, update both files.

### Expanding the Marketplace

- **Add skill:** Create `skills/skill-name/Skill.md` — auto-discovered
- **Add command:** Create `commands/command-name.md` — auto-discovered
- **Add hook:** Add script to `hooks/`, wire in both `settings.json` and `hooks/hooks.json`
- **Bump version:** Update `version` in `marketplace.json` plugin entry — required for users to get updates
- **Validate:** `claude plugin validate .` from repo root
- **Test locally:** `/plugin marketplace add ./` then `/plugin install talents@talent-tree`

### Limitations

- `strict: false` — marketplace entry defines all components, no `plugin.json` needed
- Entire `./claude/.claude` directory gets copied to plugin cache (extra files are inert)
- Plugin consumers don't get rules or settings — those go in their own Claude.md/settings

## Ledger

- v1.4: Added keybindings.json as LOCAL ONLY — per-user shortcuts wire `command:*` to skills (e.g. copy-plan-path, copy-shaping-dir)
- v1.3: Added agents/ to architecture tree and plugin exclusion list -- agents are local-only, not distributed
- v1.2: Adopted Why/What/How template with Requirements/Boundaries/Ledger
- v1.1: Added plugin marketplace with `strict: false` manifest
