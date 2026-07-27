# WHY

Swappable Claude Code configuration directories for jobs that need different settings, Rules, Skills, Agents, Commands, Hooks, or credentials from the default Claude Code setup.

# Facts

- A profile is a directory at `packages/claude/profiles/<name>/`.
- A profile becomes Claude Code's `CLAUDE_CONFIG_DIR` when launched through its alias.
- `scripts/stow.py` pre-creates `~/.claude/profiles/<name>/` as the real target directory for each profile.
- Profile credentials, history, and runtime state live under `~/.claude/profiles/<name>/`.
- Each profile config slot is either a symlink to the repo default or a real override file.
- `scripts/hooks.py` writes the Hooks declaring `roots: "all"` into every profile's own `settings.json`, so a guard that must hold everywhere is generated rather than pasted.
- Claude Code reads profile-local Skills from `skills/`.
- Claude Code reads profile-local Agents from `agents/`.
- `codex-run` reads them too, searching the active profile's `agents/` before the shared roster, so a profile Agent runs on both Harnesses and a name the profile and the shared roster both hold runs as the profile's.
- `scripts/sync.py` generates each profile's codex artifacts into its own `agents/`, and they are gitignored through `packages/claude/.gitignore`.
- Claude Code reads profile-local Commands from `commands/`.
- The filesystem is the profile selection mechanism for Skills, Agents, and Commands.
- Profile aliases live in `packages/zsh/.zshrc` and set `CLAUDE_CONFIG_DIR`.
