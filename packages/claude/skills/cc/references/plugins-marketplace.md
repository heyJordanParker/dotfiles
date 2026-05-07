
# Plugins & Marketplace

Distribute skills, commands, hooks, agents, MCP servers, and LSP servers via the plugin system.

## Concepts

- **Plugin** — self-contained directory of components (skills, commands, hooks, agents, MCP/LSP servers)
- **Marketplace** — catalog (`marketplace.json`) listing plugins and where to fetch them
- **Plugin manifest** (`plugin.json`) — optional metadata file inside a plugin
- **`strict: false`** — marketplace entry defines all components, no `plugin.json` needed

## Plugin Components

Supported in plugin manifest/marketplace entry:
- `skills` — string|array of paths
- `commands` — string|array of paths
- `agents` — string|array of paths
- `hooks` — string|array|object (paths or inline config)
- `mcpServers` — string|array|object
- `lspServers` — string|array|object
- `outputStyles` — string|array

**Not supported:** rules, settings, Claude.md files

## Marketplace File

Location: `.claude-plugin/marketplace.json` at repo root

```json
{
  "name": "marketplace-name",
  "owner": { "name": "Name", "email": "email" },
  "plugins": [{
    "name": "plugin-name",
    "source": "./path/to/plugin",
    "description": "What it does",
    "version": "1.0.0",
    "strict": false,
    "skills": ["./skills/"],
    "commands": ["./commands/"],
    "hooks": "./hooks/hooks.json"
  }],
  "metadata": {
    "description": "Marketplace description",
    "version": "1.0.0"
  }
}
```

## Plugin Sources

- **Relative path** — `"./plugins/my-plugin"` (within same repo)
- **GitHub** — `{"source": "github", "repo": "owner/repo", "ref": "v1.0", "sha": "..."}`
- **Git URL** — `{"source": "url", "url": "https://gitlab.com/team/plugin.git"}`
- **npm** — `{"source": "npm", "package": "@scope/pkg", "version": "^1.0"}`
- **Settings inline** — `{"source": "settings"}` — declare plugin entries directly in settings.json

## Hooks in Plugins

Plugin hooks live in `hooks/hooks.json` (auto-discovered) or inline in manifest. Use `${CLAUDE_PLUGIN_ROOT}` for all script paths:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "${CLAUDE_PLUGIN_ROOT}/hooks/my-script.sh"
      }]
    }]
  }
}
```

`hooks/hooks.json` is only loaded when the directory is treated as a plugin — not from `~/.claude/hooks/`.

## Distribution

- **Host on GitHub** (recommended): users add with `/plugin marketplace add owner/repo`
- **Private repos**: works if user has git credentials; set `GITHUB_TOKEN` for auto-updates
- **Team defaults**: add to `.claude/settings.json` `extraKnownMarketplaces` + `enabledPlugins`
- **CLI:** `--plugin-dir` accepts one path per flag. Repeat for multiple directories: `--plugin-dir ./a --plugin-dir ./b`
- **Seed dir:** `CLAUDE_CODE_PLUGIN_SEED_DIR` supports multiple directories separated by platform path delimiter (`:` on Unix, `;` on Windows)

## Version Management

- Bump `version` in marketplace entry or `plugin.json` (manifest wins if both set)
- Without version bump, users don't get updates (cached)
- Use `ref`/`sha` pinning for release channels (stable vs latest)

## Persistent State

`${CLAUDE_PLUGIN_DATA}` — directory for plugin state that survives updates. `/plugin uninstall` prompts before deleting it. Use for caches, user preferences, or other data that should persist across plugin versions.

## Validation & Testing

```bash
# Validate marketplace (checks skill/agent/command frontmatter + hooks/hooks.json)
claude plugin validate .

# Test locally
/plugin marketplace add ./path/to/repo
/plugin install plugin-name@marketplace-name

# Debug loading issues
claude --debug
```

## Common Patterns

### Strict Mode

- `strict: true` (default) — `plugin.json` is authority, marketplace supplements
- `strict: false` — marketplace entry is entire definition, no `plugin.json` needed

Use `strict: false` when plugin directory is not dedicated to being a plugin (e.g., a `.claude` config dir inside a dotfiles repo).

### Dual Hooks (Local + Plugin)

When your plugin source is also your local config:
- `settings.json` hooks → local use (all hooks including environment-specific)
- `hooks/hooks.json` → plugin consumers (portable hooks only)
- Hook scripts are shared, wiring is duplicated

## References

- [Official marketplace docs](https://code.claude.com/docs/en/plugin-marketplaces.md)
- [Plugin reference](https://code.claude.com/docs/en/plugins-reference.md)
- [Plugin creation](https://code.claude.com/docs/en/plugins.md)
