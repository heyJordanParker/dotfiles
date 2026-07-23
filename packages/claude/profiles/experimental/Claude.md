# WHY

Experimental Claude Code profile for trying configuration changes while inheriting the default shared Agents, Commands, and Skills.

# Facts

- `agents` is a symlink to `../../agents`.
- `commands` is a symlink to `../../commands`.
- `skills` is a symlink to `../../skills`.
- `settings.json` defines an empty `hooks` object.
- `settings.json` sets `skipDangerousModePermissionPrompt` to `true`.
- `settings.json` enables the `honcho` memory plugin from the `plastic-labs/claude-honcho` marketplace.
