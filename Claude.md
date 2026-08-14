# WHY

Reproducible macOS environment setup and Claude Code plugin distribution from a single repository: GNU Stow lays every package down at its target, and the `claude` package doubles as the plugin marketplace source.

# Facts

- `/Domain.md` is the shared vocabulary for Prompts.
- The Prompt Architecture lives in `docs/architecture/Architecture.md`.
- Decisions live in `docs/architecture/decisions/`.
- This repository's Rules live in `.claude/rules/`.
- `packages/agents` is the source of truth for shared Skills, Agents, Commands, and Hooks.
- The `claude` package stows to `~/.claude/`, where its `Claude.md` becomes the user-global Claude.md.
- `packages/claude`'s `agents`, `commands`, and `skills` are symlinks into `packages/agents`.
- `scripts/sync.py` restows packages, generates Codex Agent artifacts, and generates Hook wiring.
- Plugin packaging dereferences `packages/claude` symlinks into real files in the plugin cache.
- Plugin consumers get Skills, Commands, and the five shell Hooks, not Rules, settings, or Agents.
- `packages/codex/Agents.md` points at `.claude/Claude.md`, so Codex loads the same user-global Claude.md as Claude Code after stow.
- `~/Developer/references` holds repositories cloned to read and delete when done.
- `~/Developer/services` holds repositories cloned and run by `setup.sh`.
- The `drawbridge` service is the live Excalidraw diagram server behind `/diagram`.
