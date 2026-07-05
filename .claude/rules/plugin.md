---
paths:
  - "packages/claude/**"
  - ".claude-plugin/**"
---

### Keep local config out of plugin distribution
Never commit `settings.json`, `Claude.md`, or other local config to plugin distribution. Local config stays local. Plugin consumers get Skills, Commands, and Hooks; never Rules or settings.

### Keep Rules out of the plugin manifest
Never put Rules in the plugin manifest. The plugin schema does not support them.

IF adding a Skill or Command:
### Add it under `packages/agents`
The source of truth is `packages/agents/skills/<name>/SKILL.md` with the uppercase manifest name that Codex byte-matches, or `packages/agents/commands/<name>.md`. The stow links and the committed `claude/skills → ../agents/skills` / `claude/commands → ../agents/commands` symlinks distribute it everywhere.

IF adding or changing a Hook:
### Generate Hook wiring from `BINDING`
The Python module in `packages/agents/hooks/` declares `BINDING` with events and `harness: all|claude|codex`. Run `python3 scripts/sync.py` to generate the wiring. Never hand-edit the managed entries in `settings.json` or the `[[hooks.*]]` + `[hooks.state]` regions of `config.toml`.

IF adding or changing a plugin-distributed Hook:
### Update the shell Hook copy with the Python Hook
A plugin-distributed Hook also needs its shell copy under `packages/claude/hooks/` referenced in `hooks/hooks.json`. Update Python source and shell copy together.

IF releasing plugin changes:
### Bump, validate, and install locally
Bump `version` in `marketplace.json`, run `claude plugin validate .` from the repo root, and test locally with `/plugin marketplace add ./` then `/plugin install talents@talent-tree`. Consumers get updates only on a bump.
