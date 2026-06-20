# Claude Code Profiles

## Why

One Claude install, many jobs. A copywriting session wants different rules, skills, and a warmer system prompt than an experiment that wants the full agent harness with everything stripped back. A profile is a whole swappable `CLAUDE_CONFIG_DIR` — its own rules, skills, agents, MCP servers, hooks, settings, and credentials — launched by its own `cld`-style alias. The default `cld` setup stays the baseline; a profile diverges from it only where the job needs it to.

Profiles inherit by default and override by exception. Each config slot is either a symlink back to the repo default (inherit) or a real file (override), so a near-default profile is a directory of symlinks plus the one thing it changes, and a fully-custom profile is a directory of real files. The cost of a profile scales with how much it actually changes, not with the size of the config surface.

## What

A profile is a directory at `packages/claude/profiles/<name>/`, committed to the repo, that stows alongside the `claude` package. It becomes Claude's `CLAUDE_CONFIG_DIR` when launched through its alias, so Claude reads every config slot — settings, rules, skills, agents, commands, system prompt, MCP servers — from the profile instead of from `~/.claude`.

### Requirements

- Each profile lives at `packages/claude/profiles/<name>/`, committed, and stows with the `claude` package. Its launch target is the real directory `~/.claude/profiles/<name>/`, pre-created by `scripts/stow.py` so Claude's runtime files (credentials, history, project state) land under `~`, never inside the tracked repo
- Each config slot in a profile is either a symlink to the repo default (inherit) or a real file (override). A `settings.json` symlinked to `../../settings.json` inherits the default hooks and env; a real `settings.json` diverges
- Skills, agents, and commands are filesystem-scoped — Claude reads them from the profile's own `skills/`, `agents/`, `commands/` dirs. Three shapes per dir: **all** (a single `skills -> ../../skills` symlink), **partial** (a real `skills/` dir of per-entry symlinks to `../../skills/<name>`), **none** (omit the dir). There is no per-launch skill-subset flag — the filesystem is the selection
- Each profile gets its own alias in `packages/zsh/.zshrc`, setting `CLAUDE_CONFIG_DIR` to the profile's target. The existing `cld` alias is the reference shape
- Each profile authenticates once with its own `claude /login`; the credential persists in the profile's target dir under `~`, outside the repo

### Boundaries

- Never write a profile's credentials, history, or runtime state into the tracked repo — they belong in the `~/.claude/profiles/<name>/` target, which is why `scripts/stow.py` pre-creates it as a real dir
- Never reach for a per-launch skill-subset flag — scope skills/agents/commands by which symlinks the profile's dir contains
- Never copy the default config into a profile when a symlink inherits it — a real file means "this profile diverges here", and a copy that drifts from the default is the failure this distinction exists to prevent

## How

### The two ends of the spectrum

**Fully-overriding profile** — its own everything, inheriting nothing. Every slot is a real file; no `skills/`, `agents/`, or `commands/` symlinks to the defaults.

```
packages/claude/profiles/copywriting/
├── Claude.md            # real — the copywriting rules and system context
├── settings.json        # real — its own hooks/env (or empty hooks to run bare)
├── skills/              # real dir of skills this profile owns
│   └── house-style/SKILL.md
├── agents/              # real dir of its own agents (or omitted)
└── .mcp.json            # real — only the MCP servers this job needs
```

Launched with `--mcp-config .mcp.json --strict-mcp-config` so only those servers load, and either no `--agent` flag (default system prompt) or `--agent <name>` for a chosen one.

**Near-default profile** — inherits skills, agents, and settings; overrides one thing. The directory is mostly symlinks plus the single real file that diverges.

```
packages/claude/profiles/experiment/
├── Claude.md            # real — the only override: this experiment's rules
├── settings.json -> ../../settings.json    # inherit default hooks/env
├── skills -> ../../skills                   # inherit all skills
├── agents -> ../../agents                   # inherit all agents
└── commands -> ../../commands               # inherit all commands
```

(All relative symlinks resolve from `packages/claude/profiles/<name>/`, so `../../` reaches `packages/claude/`.)

### Adding a profile

1. Create `packages/claude/profiles/<name>/`.
2. For each slot, decide inherit or override:
   - **Inherit a single file** (`settings.json`): `ln -s ../../settings.json settings.json`.
   - **Override a single file**: write the real file.
   - **Inherit all** of skills/agents/commands: `ln -s ../../skills skills` (likewise `agents`, `commands`).
   - **Inherit some**: make a real dir and symlink the chosen entries — `mkdir skills && ln -s ../../../skills/<name> skills/<name>` (three `../` — the symlink sits one level deeper inside the real `skills/` dir).
   - **Override**: write real files under the dir.
   - **None**: omit the dir.
3. Add the alias to `packages/zsh/.zshrc`, modeled on `cld`, setting `CLAUDE_CONFIG_DIR=~/.claude/profiles/<name>`. Scope MCP servers with `--mcp-config <file> --strict-mcp-config` (only those load) — a committed `.mcp.json` in the profile works when the servers carry no secret; servers with a secret key or OAuth must be added once per profile with `claude mcp add -s user`, which lands them in the profile's own (untracked) `.claude.json`. Pick the system prompt with `--agent <name>` or leave it off for the default.
4. Run `python3 scripts/sync.py`. `scripts/stow.py` discovers every dir under `packages/claude/profiles/`, pre-creates its `~/.claude/profiles/<name>/` real target, and stows the profile's children into it — no per-profile `stow.py` edit.
5. Run `claude /login` once inside the profile (launch via its alias first) to authenticate it; the credential persists in the profile's target dir.

### Customizing an existing profile

A profile is partial-to-complete on a per-slot basis — change any slot's inherit/override independently:

- **Inherit → override**: replace the symlink with a real file (`rm settings.json && $EDITOR settings.json`), then restow.
- **Override → inherit**: delete the real file and symlink the default, then restow.
- **Widen/narrow a skill set**: switch the `skills` slot between the single `-> ../../skills` symlink (all) and a real dir of per-entry symlinks (partial), then restow.

Restow only on these filesystem-shape changes (adding/removing/renaming a slot); editing the *content* of an already-stowed real file flows through the live symlink with no stow command.
