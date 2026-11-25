# WHY

Reproducible macOS environment setup and Claude Code plugin distribution from a single repository: GNU Stow lays every package down at its target, and the `claude` package doubles as the plugin marketplace source.

# Facts

- The Prompt Architecture lives in `docs/architecture/Architecture.md`.
- Decisions live in `docs/architecture/decisions/`.
- This repository's Rules live in `.claude/rules/`.
- `packages/agents` is the source of truth for shared Skills, Agents, Commands, and Hooks.
- The `claude` package stows to `~/.claude/`, where its `Claude.md` becomes the user-global Claude.md.
- `packages/claude`'s `agents`, `commands`, and `skills` are symlinks into `packages/agents`.
- The `rulebook` package stows to `~/.config/rulebook/`: the global rulebook config, the rules it checks every Prompt with, and their tests.
- `scripts/sync.py` restows packages and generates Hook wiring.
- Plugin packaging dereferences `packages/claude` symlinks into real files in the plugin cache.
- Our codex settings and Hook wiring live in `packages/codex-system/config.toml`, stowed to `/etc/codex`, codex's read-only system layer. `~/.codex/config.toml` is codex's own local file, never committed, where it writes project trust and app state.
- `packages/codex/Agents.md` points at `.claude/Claude.md`, so Codex loads the same user-global Claude.md as Claude Code after stow.
- The Brewfile installs `moshi-hook`; `setup.sh` starts its daemon through `brew services` and enables Remote Login, `moshi` serves the Moshi Desktop web client on `127.0.0.1:24544`, and `moshi-hook host setup` prints an Easy Pair QR code that only the Architect can scan.
- `setup.sh` installs bun with Bun's own installer at `~/.bun/bin/bun`, never Homebrew. Homebrew installs every program read-only, and a read-only bun leaves an 88 MB copy of itself in the working folder after every `bun build --compile` (oven-sh/bun pull request 46624). The installer runs with `SHELL=/bin/sh`, so it writes no line into the stowed `.zshrc`. `.zsh_completions.zsh` holds bun's completions, and `bun upgrade` updates it.
- `~/Developer/references` holds repositories cloned to read and delete when done.