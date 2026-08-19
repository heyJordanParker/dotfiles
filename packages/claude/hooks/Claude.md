# WHY

Plugin-distributed shell Hook surface for external Claude Code installs that have no local Python Hook layer.

# Facts

- Live Python Hooks live in `packages/agents/hooks/`.
- Claude Code wires live Python Hooks by absolute `~/.agents/hooks/<module>.py` paths in `settings.json`.
- Codex wires live Python Hooks in `config.toml`.
- `hooks.json` is the plugin Hook wiring file.
- `hooks.json` references exactly five shell Hooks: `block-git-revert.sh`, `block-unsafe-delete.sh`, `validate-planning-docs.sh`, `validate-plan-quality.sh`, and `sync-shaping.sh`.
- `herdr-agent-state.sh` is a third-party vendor script for agent-state notifications.
- The Python Hook session state lives in `packages/agents/hooks/lib/session_state.py`.
- `record_session_event.py` owns state-recording events for the Python Hook layer.
- Plugin users get the tracer binary as a Command, not tracer Hooks.
