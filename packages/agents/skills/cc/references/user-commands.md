# User Commands

Built-in slash commands for Claude Code.

## Session & History

- **/clear** — Clear conversation history
- **/compact [instructions]** — Compress context, optionally focus on specific topics
- **/rewind** — Roll back context and code state (also `Esc+Esc`)
- **/resume <name>** — Resume session by name
- **/rename** — Name current session (also: `claude -n <name>` to name at startup)
- **/branch [name]** — Fork current session (alias: **/fork**). New session ID, same history up to branch point. Also: `claude --resume <id> --fork-session`
- **/btw <question>** — Ephemeral side query without pausing agent. Full conversation visibility, no tools, never enters history. Dismiss with Space/Enter/Escape
- **/exit** — End session

## Automation

- **/loop [interval] <prompt>** — Schedule recurring prompt in current session. Default 10m. Supports `s/m/h/d` units
- **/schedule [description]** — Create cloud-based scheduled task (survives restarts, runs when machine is off). Min 1h interval
- **/batch <instruction>** — Research codebase, decompose into 5-30 independent units, spawn one background agent per unit, each opens a PR. Requires git repo

### /loop Timing

- Cron-based, 1-minute minimum granularity
- Fires on interval regardless of whether previous run completed
- Each firing is a new message into the session
- Session-scoped — runs within a single persistent session, not spawning fresh sessions
- If agent is mid-turn when cron fires, the message queues and gets processed when current turn ends
- 3-day auto-expiry, max 50 tasks per session
- Jitter: up to 10% of period late, capped at 15 minutes
- Disable: `CLAUDE_CODE_DISABLE_CRON=1`

## Configuration

- **/config** — Open settings panel (searchable)
- **/model** — Switch AI model
- **/effort** — Set model effort level (low/medium/high/max)
- **/theme** — Open theme picker (`Ctrl+T` toggles syntax highlighting)
- **/keybindings** — Configure keyboard shortcuts
- **/permissions** — View/update tool permissions
- **/allowed-tools** — Configure tool permissions interactively
- **/vim** — Enable vim-style editing

## Context & Memory

- **/context** — Visualize context usage
- **/memory** — Edit CLAUDE.md memory files
- **/init** — Initialize project with CLAUDE.md
- **/add-dir** — Add working directories

## Usage & Status

- **/help** — Show all available commands
- **/doctor** — Check installation health, update channel
- **/status** — View account and system status
- **/usage** — Show plan limits and rate status
- **/cost** — Show token usage statistics
- **/stats** — Usage streak, favorites, graph (`r` cycles date range)

## Extensibility

- **/hooks** — Manage automation hooks
- **/mcp** — Manage MCP servers
- **/mcp enable <name>** — Enable specific server
- **/agents** — Create, edit, list agents
- **/skills** — Access skills
- **/plugins** — Plugin management
- **/plugins discover** — Discover available plugins

## Account

- **/login** — Switch Anthropic accounts
- **/logout** — Sign out

## Development Workflow

- **/review** — Request code review
- **/plan** — Enter plan mode
- **/todos** — View tracked TODO items
- **/tasks** — Background tasks dialog
- **/pr_comments** — View pull request comments
- **/sandbox** — Enable sandboxed bash
- **/voice** — Toggle voice input mode (shows dictation language on enable, warns if `language` setting is unsupported for voice input)

## Keyboard Shortcuts

- `Ctrl+O` — Enter transcript mode; press `/` to search, `n`/`N` to step through matches
- `Ctrl+X Ctrl+E` — Open external editor (alias for `Ctrl+G`)
- `Ctrl+X Ctrl+K` — Stop all background agents

## Session Transfer

For claude.ai subscribers:

- **/teleport** — Resume session at claude.ai/code
- **/remote-control** (or **/rc**) — Enable remote control (connect from claude.ai/code or mobile app). Also: `claude --rc` flag or `claude remote-control` server mode
- **/remote-env** — Configure remote sessions

## Setup & Integrations

- **/terminal-setup** — Install Shift+Enter keybinding
- **/install-github-app** — GitHub Actions integration for PR reviews

## Utilities

- **/bug** — Report bugs (sends conversation to Anthropic)
- **/copy [N]** — Copy last assistant response (optional index N copies Nth-latest)
- **/feedback** — Generate GitHub issue URL
- **/export [filename]** — Export conversation
- **/tag** — Tag sessions

## CLI Flags

Noteworthy flags not covered elsewhere:

- `--bare` — Minimal mode: skip hooks, skills, plugins, MCP servers, auto memory, CLAUDE.md auto-discovery. Auth must come from `ANTHROPIC_API_KEY` or `apiKeyHelper` via `--settings` (OAuth/keychain skipped). Use `--append-system-prompt`, `--mcp-config`, `--settings`, `--agents`, `--plugin-dir` to explicitly load context. Will become default for `-p` in a future release
- `--agent <name>` — Start session using a custom agent's system prompt, tools, and model. See [writing-agents.md](writing-agents.md) for details
- `--add-dir <dirs...>` — Add working directories (space-separated). Skills in added dirs' `.claude/skills/` auto-discovered. CLAUDE.md from added dirs NOT loaded by default (set `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` to enable)
- `--rc [name]` — Start session with remote control enabled. Different from `claude remote-control` (server mode, no local REPL)
- `--fork-session` — Use with `--resume` or `--continue` to create a new session ID instead of appending to original. CLI equivalent of `/branch`
- `--channels` — Allow MCP servers to push messages into session (research preview)
- `--chrome` — Enable Chrome extension integration for frontend work (beta, Chrome/Edge only, requires v2.0.73+)
- `-w, --worktree [name]` — Create a new git worktree for this session

## Custom Skills as Commands

Skills automatically become slash commands. Create in:
- **Project:** `.claude/skills/skill-name/SKILL.md`
- **Personal:** `~/.claude/skills/skill-name/SKILL.md`

Directory name becomes command: `review/SKILL.md` → `/review`

Run `/help` to see all commands including custom skills.
