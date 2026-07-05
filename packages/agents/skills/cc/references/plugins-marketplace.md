# Plugins & Marketplace

The Process for shipping this setup to another repo through the Claude Code plugin marketplace.

## 1. Name what the plugin ships

- A plugin is a self-contained directory of Skills, Commands, Agents, Hooks, Model Context Protocol servers, Language Server Protocol servers, output styles, themes, monitors, or executable files.
- The marketplace is `.claude-plugin/marketplace.json`, the catalog that names plugins and their sources.
- The plugin manifest is `plugin.json`, the optional metadata file inside a plugin.
- `strict: true` makes `plugin.json` the authority and the marketplace entry supplemental.
- `strict: false` makes the marketplace entry the whole plugin definition, with no `plugin.json` required.
- Plugin manifest and marketplace entries accept `skills`, `commands`, `agents`, `hooks`, `mcpServers`, `lspServers`, `outputStyles`, `themes`, and `monitors`.
- `themes` and `monitors` belong under `"experimental": { ... }`; Claude Code v2.1.129 still accepts top-level values, but `claude plugin validate` warns.
- Plugins can ship executable files under `bin/` and invoke them as bare commands from the Bash tool since Claude Code v2.1.91.
- A plugin with a root-level `SKILL.md` and no `skills/` subdirectory is surfaced as a Skill since Claude Code v2.1.142.
- A Skill declared with `"skills": ["./"]` uses the Skill frontmatter `name` for the invocation name since Claude Code v2.1.94.

### Keep Rule Files, settings, and Claude.md out of plugin distribution
Claude Code plugins do not support Rule Files, settings, or Claude.md files as plugin components. This repository's plugin consumers get Skills, Commands, and the plugin-distributed shell Hooks; Rules, settings, and Claude.md stay local.
Example: `packages/claude/rules/` stays local while `packages/claude/skills` ships through `"skills": ["./skills/"]`.
Never: list `rules/`, `settings.json`, or `Claude.md` in `.claude-plugin/marketplace.json`.

## 2. Write the marketplace entry

- This repository's marketplace file lives at `.claude-plugin/marketplace.json`.
- This repository's plugin source is `./packages/claude`.
- A relative path source points inside the same repository, such as `"./plugins/my-plugin"`.
- A GitHub source uses `{ "source": "github", "repo": "owner/repo", "ref": "v1.0", "sha": "..." }`.
- A Git source address uses `{ "source": "url", "url": "https://gitlab.com/team/plugin.git" }`.
- An npm source uses `{ "source": "npm", "package": "@scope/pkg", "version": "^1.0" }`.
- A settings inline source uses `{ "source": "settings" }` to declare plugin entries directly in settings.json.
- GitHub and Git source addresses accept `"skipLfs": true` to skip Git Large File Storage downloads during clone and update since Claude Code v2.1.153.

### Use `strict: false` when the plugin source is also local Claude Code configuration
A directory that exists for local stow configuration is not dedicated to being a plugin, so the marketplace entry owns the plugin definition.
Template:
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
Example: this repository's marketplace entry uses `"source": "./packages/claude"`, `"strict": false`, `"skills": ["./skills/"]`, `"commands": ["./commands/"]`, and `"agents": []`.
Never: add `plugin.json` just to repeat fields already owned by the `strict: false` marketplace entry.

## 3. Pick the install path

- Users add a GitHub-hosted marketplace with `/plugin marketplace add owner/repo`.
- Private repositories work when the User has git credentials; `GITHUB_TOKEN` enables auto-updates.
- Team defaults live in `.claude/settings.json` through `extraKnownMarketplaces` and `enabledPlugins`.
- `--plugin-dir` loads a plugin from a directory or `.zip` for the session, and is repeatable since Claude Code v2.1.128.
- `--plugin-url <url>` fetches a plugin `.zip` from a web address for the session, and is repeatable since Claude Code v2.1.129.
- `CLAUDE_CODE_PLUGIN_SEED_DIR` accepts multiple directories separated by the platform path delimiter: `:` on Unix and `;` on Windows.

### Use session-only loading for testing, not distribution
`--plugin-dir` and `--plugin-url` prove the plugin loads in the current session; they are not the install path for plugin Users.
Example: `claude --plugin-dir ./packages/claude` tests local loading for one session.
Never: document `--plugin-dir` as the way another repository should install this marketplace.

## 4. Wire plugin Hooks

- Plugin Hooks live in `hooks/hooks.json` or inline in the plugin manifest.
- `hooks/hooks.json` is auto-discovered only when the directory is treated as a plugin; Claude Code does not load it from `~/.claude/hooks/`.
- Plugin command configs can reference `${CLAUDE_PROJECT_DIR}` alongside `${CLAUDE_PLUGIN_ROOT}`.
- Standard input/output Model Context Protocol servers declared by the plugin receive `CLAUDE_PROJECT_DIR` in their environment since Claude Code v2.1.139.

### Use `${CLAUDE_PLUGIN_ROOT}` for every plugin Hook command path
Plugin Users install to different paths, so shell Hook commands must resolve from the plugin root.
Template:
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
Example: `packages/claude/hooks/hooks.json` calls `${CLAUDE_PLUGIN_ROOT}/hooks/block-git-revert.sh`.
Never: call `~/.claude/hooks/block-git-revert.sh` from plugin Hook wiring.

IF your plugin source is also your local Claude Code configuration:
### Keep local Hook wiring and plugin Hook wiring separate
The live Hooks are Python files in `packages/agents/hooks/`; their local wiring in settings.json and Codex config.toml is generated from each Hook's `BINDING`. Plugin consumers get maintained shell copies wired by `packages/claude/hooks/hooks.json`, so changing a plugin-distributed Hook means updating the Python source and its shell copy.
Example: a change to unsafe delete blocking updates `packages/agents/hooks/block_unsafe_delete.py` and `packages/claude/hooks/block-unsafe-delete.sh`.
Never: update the shell Hook only and leave the local Python Hook behavior behind.

## 5. Manage versions and plugin state

- The plugin `version` can live in the marketplace entry or `plugin.json`; the manifest wins when both set it.
- Without a `version` bump, installed Users do not get updates because Claude Code caches plugin installs.
- `defaultEnabled: false` in `plugin.json` or a marketplace entry ships a plugin disabled until the User enables it through `/plugin` or `claude plugin enable`; dependencies of enabled plugins still enable automatically since Claude Code v2.1.154.
- A marketplace `renames` map auto-renames installed plugins and updates the User's settings to the new name since Claude Code v2.1.193.
- `claude plugin disable` refuses while another enabled plugin depends on the target, and `claude plugin enable` force-enables transitive dependencies since Claude Code v2.1.143.
- `${CLAUDE_PLUGIN_DATA}` is the directory for plugin state that survives updates; `/plugin uninstall` prompts before deleting it.

### Bump `version` for every distributed change
The version is what makes Claude Code fetch an update for installed plugin Users.
Example: change `.claude-plugin/marketplace.json` from `"version": "1.5.0"` to `"version": "1.5.1"` when the marketplace ships a changed Skill.
Never: rely on changed files alone to reach installed plugin Users.

### Put plugin state that survives updates under `${CLAUDE_PLUGIN_DATA}`
Plugin state stored beside the plugin files can be overwritten by update and removed by reinstall.
Example: use `${CLAUDE_PLUGIN_DATA}/cache.json` for a plugin cache.
Never: write User state into `${CLAUDE_PLUGIN_ROOT}`.

## 6. Validate and test the plugin

- A top-level `$schema` key is accepted in `marketplace.json` and `plugin.json` since Claude Code v2.1.120.
- `claude plugin` subcommands include `init`, `list`, `details`, `enable`, `disable`, `install`, `uninstall`, `prune`, `tag`, `update`, `validate`, and `marketplace`.
- `claude plugin list` accepts `--enabled` and `--disabled`.
- `claude plugin details <name>` shows component inventory and projected token cost.
- `claude plugin uninstall --prune` cascades through removable auto-installed dependencies.
- `claude plugin init` scaffolds a new plugin at `~/.claude/skills/<name>/` and auto-loads the next session as `<name>@skills-dir`.
- `/plugin list` is the in-session equivalent of `claude plugin list` since Claude Code v2.1.163.
- Official marketplace docs are at <https://code.claude.com/docs/en/plugin-marketplaces.md>.
- The plugin reference is at <https://code.claude.com/docs/en/plugins-reference.md>.
- The plugin creation docs are at <https://code.claude.com/docs/en/plugins.md>.

### Run Claude Code plugin commands before calling plugin distribution done
Files existing under `packages/claude` do not prove that Claude Code loads the plugin components.
Example: run `claude plugin validate .`, then `/plugin marketplace add ./path/to/repo`, then `/plugin install plugin-name@marketplace-name`.
Example: run `claude --debug` when plugin loading fails.
Never: claim a component ships because it exists under `packages/claude` without validating plugin loading.
