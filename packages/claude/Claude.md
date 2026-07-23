# WHY

Claude Code user-global configuration and plugin marketplace source, kept in the same stow package so the local Harness setup and distributed Claude Code plugin come from one tree.

# Facts

- The `claude` package stows to `~/.claude/`.
- `Claude.md` becomes Claude Code's user-global Claude.md after stow.
- `settings.json` is local Claude Code configuration.
- `keybindings.json` is local Claude Code keybinding configuration.
- `rules/` holds the Rule Files loaded in every session.
- @Domain.md is the shared vocabulary for working with Agents.
- The Prompt Architecture lives in `docs/architecture/Architecture.md`.
- `agents` is a symlink to `../agents/agents`.
- `commands` is a symlink to `../agents/commands`.
- `skills` is a symlink to `../agents/skills`.
