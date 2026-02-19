# Dotfiles

GNU Stow-managed dotfiles. Each subdirectory is a package.

## Packages

!`ls -d */ | grep -v '^\.'`

## After Editing

After modifying files in any package, restow it:
```bash
stow -R <package>   # e.g., stow -R zsh
```

## Adding New Packages

1. Create package directory: `mkdir <package>`
2. Mirror the target structure inside:
   ```
   <package>/
   └── .config/
       └── <app>/
           └── config.toml
   ```
3. Stow it: `stow <package>`

## Installing CLI Tools

When adding a new CLI tool:
1. Add to `Brewfile` (appropriate section)
2. If config needed: create package dir, add config files
3. If wrapper needed (secrets, env vars): add to `bin/.local/bin/`
4. Stow any new/modified packages: `stow -R <package>`

## Python Tools (pipx)

For Python CLI tools, use pipx (isolated venvs):
```bash
pipx install <package>
```

## Plugin Marketplace

This repo doubles as a Claude Code plugin marketplace via `.claude-plugin/marketplace.json`. Users install with:
```
/plugin marketplace add heyJordanParker/dotfiles
/plugin install talents@talent-tree
```

### Structure

```
dotfiles/
├── .claude-plugin/
│   └── marketplace.json       # marketplace catalog
└── claude/.claude/            # plugin root (source: ./claude/.claude)
    ├── skills/                # auto-discovered by plugin
    ├── commands/              # auto-discovered by plugin
    ├── hooks/
    │   ├── hooks.json         # plugin hook wiring (non-tmux only)
    │   ├── session-start.md   # shared hook content
    │   ├── block-git-revert.sh
    │   └── ...
    ├── settings.json          # LOCAL ONLY — not used by plugin
    ├── Claude.md              # LOCAL ONLY — not used by plugin
    └── rules/                 # NOT SUPPORTED by plugins
```

### What Gets Distributed

- **Skills** — all `skills/` subdirectories
- **Commands** — all `commands/*.md` files
- **Hooks** — wired via `hooks/hooks.json` (uses `${CLAUDE_PLUGIN_ROOT}` paths)

### What Does NOT Get Distributed

- **Rules** — no `rules` field in plugin manifest schema
- **Settings** — no `settings` field in plugin manifest schema
- `settings.json`, `Claude.md`, tmux hooks — exist locally via stow, ignored by plugin

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
