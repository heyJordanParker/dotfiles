# WHY

Plugin-distributed shell Hook surface for external Claude Code installs that have no local Python Hook layer.

# Facts

- Live Python Hooks live in `packages/agents/hooks/`.
- Claude Code wires live Python Hooks by absolute `~/.agents/hooks/<module>.py` paths in `settings.json`.
- Codex wires live Python Hooks in `config.toml`.
- `hooks.json` is the plugin Hook wiring file.
- `hooks.json` references exactly three shell Hooks: `block-git-revert.sh`, `block-unsafe-delete.sh`, and `sync-shaping.sh`.
- `herdr-agent-state.sh` is herdr's vendored Claude integration. It reports each session's identity to herdr at `SessionStart`, and herdr reads Claude's working, idle, and blocked state from the screen.
- The Python Hook session state lives in `packages/agents/hooks/lib/session_state.py`.
- `record_session_event.py` owns state-recording events for the Python Hook layer.
