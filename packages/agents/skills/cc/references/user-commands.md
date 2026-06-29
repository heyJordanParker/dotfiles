# User Commands

Built-in slash commands for Claude Code.

## Session & History

- **/clear** — Clear conversation history
- **/compact [instructions]** — Compress context, optionally focus on specific topics
- **/rewind** — Roll back context and code state (also `Esc+Esc`; alias **/undo**). Can resume a conversation from before `/clear` was run (v2.1.191+)
- **/recap** — Summarize what happened since you were last in the session (v2.1.108+)
- **/resume <name>** — Resume session by name
- **/rename** — Name current session (also: `claude -n <name>` to name at startup)
- **/branch [name]** — Fork current session (alias: **/fork**). New session ID, same history up to branch point. Also: `claude --resume <id> --fork-session`
- **/btw <question>** — Ephemeral side query without pausing agent. Full conversation visibility, no tools, never enters history. Dismiss with Space/Enter/Escape
- **/exit** — End session

## Automation

- **/loop [interval] <prompt>** — Schedule recurring prompt in current session (alias **/proactive**). Default 10m. Supports `s/m/h/d` units
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

- **/config** — Open settings panel (searchable). `/config key=value` sets any setting from the prompt (e.g. `/config thinking=false`); `/config --help` lists shorthand keys
- **/model** — Switch AI model
- **/effort** — Set model effort level (low/medium/high/xhigh/max); no args opens an interactive slider. `xhigh` (between high and max) applies on supported models, else falls back to high
- **/theme** — Open theme picker (`Ctrl+T` toggles syntax highlighting); supports named custom themes
- **/keybindings** — Configure keyboard shortcuts
- **/permissions** — View/update tool permissions
- **/allowed-tools** — Configure tool permissions interactively
- **/tui** — Switch the conversation renderer; `/tui fullscreen` enables flicker-free alt-screen rendering (lower memory, mouse support, copy-on-select) (v2.1.110+)
- **/focus** — Toggle focus view: shows the prompt, a one-line tool summary with edit diffstats, and the final response only (v2.1.110+)
- **/scroll-speed** — Tune mouse-wheel scroll speed with a live preview (v2.1.139+)

Vim editing is no longer a `/vim` command — toggle it via `/config` → Editor mode (v2.1.92+).

## Context & Memory

- **/context** — Visualize context usage
- **/memory** — Edit CLAUDE.md memory files
- **/init** — Initialize project with CLAUDE.md
- **/add-dir** — Add working directories
- **/cd** — Move the session to a new working directory without breaking the prompt cache (v2.1.169+)

## Usage & Status

- **/help** — Show all available commands
- **/doctor** — Check installation health, update channel
- **/status** — View account and system status
- **/usage** — Unified plan limits, rate status, and usage breakdown by category (skills, subagents, plugins, per-MCP-server cost). `/cost` and `/stats` are now typing shortcuts that open the relevant `/usage` tab (v2.1.118+)

## Extensibility

- **/hooks** — Manage automation hooks
- **/mcp** — Manage MCP servers
- **/mcp enable <name>** — Enable specific server
- **/agents** — Create, edit, list agents
- **/skills** — Access skills
- **/reload-skills** — Re-scan skill directories without restarting (v2.1.152+)
- **/reload-plugins** — Re-scan and update plugins, auto-installing missing dependencies
- **/plugins** — Plugin management (**/plugin list** lists installed plugins)
- **/plugins discover** — Discover available plugins

## Account

- **/login** — Switch Anthropic accounts
- **/logout** — Sign out

## Development Workflow

- **/review** — Request code review (uses the `/code-review medium` engine)
- **/code-review [effort]** — Report correctness bugs at a chosen effort; `--comment` posts inline PR comments, `--fix` applies findings
- **/simplify** — Cleanup-only review (reuse, simplification, efficiency, altitude) that applies the fixes (v2.1.154+)
- **/ultrareview [PR#]** — Cloud multi-agent review of the current branch or a PR (CLI: `claude ultrareview`)
- **/goal** — Set a completion condition; Claude keeps working across turns until it's met (v2.1.139+)
- **/workflows** — View and manage dynamic workflows; trigger one in-prompt with the `ultracode` keyword (v2.1.154+, renamed from `workflow` in v2.1.160)
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
- **/less-permission-prompts** — Scan transcripts for common read-only Bash/MCP calls and propose an allowlist for `.claude/settings.json` (v2.1.111+)
- **/team-onboarding** — Generate a teammate ramp-up guide from your local Claude Code usage (v2.1.101+)
- **/powerup** — Interactive lessons teaching Claude Code features with animated demos (v2.1.90+)

## CLI Flags

Noteworthy flags not covered elsewhere:

- `--bare` — Minimal mode (sets `CLAUDE_CODE_SIMPLE=1`): skip hooks, LSP, plugin sync, attribution, auto-memory, background prefetches, keychain reads, and CLAUDE.md auto-discovery. Skills still resolve via `/skill-name`. Auth is strictly `ANTHROPIC_API_KEY` or `apiKeyHelper` via `--settings` (OAuth/keychain never read); 3P providers use their own credentials. Use `--append-system-prompt`, `--mcp-config`, `--settings`, `--agents`, `--plugin-dir` to load context explicitly
- `--safe-mode` (or `CLAUDE_CODE_SAFE_MODE`) — Start with all customizations (CLAUDE.md, skills, plugins, hooks, MCP servers, commands, agents, output styles, workflows, themes, keybindings) disabled for troubleshooting; admin policy settings still apply (v2.1.169+)
- `--exclude-dynamic-system-prompt-sections` — Move per-machine sections (cwd, env, memory paths, git status) out of the system prompt into the first user message, improving cross-user prompt-cache reuse (v2.1.98+)
- `--bg, --background` — Start the session as a background agent and return immediately; manage with `claude agents` (v2.1.154+)
- `--agent <name>` — Start session using a custom agent's system prompt, tools, and model. See [writing-agents.md](writing-agents.md) for details
- `--add-dir <dirs...>` — Add working directories (space-separated). Skills in added dirs' `.claude/skills/` auto-discovered. CLAUDE.md from added dirs NOT loaded by default (set `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` to enable)
- `--rc [name]` — Start session with remote control enabled. Different from `claude remote-control` (server mode, no local REPL)
- `--fork-session` — Use with `--resume` or `--continue` to create a new session ID instead of appending to original. CLI equivalent of `/branch`
- `--channels` — Allow MCP servers to push messages into session (research preview)
- `--chrome` — Enable Chrome extension integration for frontend work (beta, Chrome/Edge only, requires v2.0.73+)
- `-w, --worktree [name]` — Create a new git worktree for this session

### Subcommands

- `claude agents` — Agent view: a single list of every Claude Code session, running/blocked/done; dispatch and attach to background sessions (v2.1.139+)
- `claude project purge [path]` — Delete all Claude Code state for a project (transcripts, tasks, file history, config entry); supports `--dry-run`, `-y`, `--all` (v2.1.126+)
- `claude mcp login <name>` / `claude mcp logout <name>` — Authenticate MCP servers from the CLI without the interactive `/mcp` menu; `--no-browser` for SSH (v2.1.186+)
- `claude ultrareview [target]` — Run `/ultrareview` non-interactively from CI; `--json` for raw output (v2.1.120+)
- `claude plugin ...` — see [plugins-marketplace.md](plugins-marketplace.md)

## Custom Skills as Commands

Skills automatically become slash commands. Create in:
- **Project:** `.claude/skills/skill-name/SKILL.md`
- **Personal:** `~/.claude/skills/skill-name/SKILL.md`

Directory name becomes command: `review/SKILL.md` → `/review`

Run `/help` to see all commands including custom skills.

The model can also discover and invoke built-in slash commands (e.g. `/init`, `/review`, `/security-review`) on its own via the Skill tool (v2.1.108+).
