# WHY

Swappable Claude Code configuration directories for jobs that need different settings, Rules, Skills, Agents, Commands, Hooks, or credentials from the default Claude Code setup.

# Facts

- A profile is the Claude Code config directory (`CLAUDE_CONFIG_DIR`) of the sessions launched on it.
- Each file or folder in a profile is a symlink to the default root's copy or the profile's own override.
- Every profile's `settings.json` sets `cleanupPeriodDays: 100000000`, because each profile keeps its own `projects/` folder and Claude Code deletes its transcripts after 30 days without it.
