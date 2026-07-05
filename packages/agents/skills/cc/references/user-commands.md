# User Commands

The Process for finding whether a built-in slash command already does the Task, and for writing `commands/<command>.md` only when it does not.

## 1. Check built-ins before writing a Command

- A Command is a Skill the Architect invokes manually.
- Side effects are the dividing line between a Skill the Agent can invoke and a Command the Architect fires.
- Built-in slash commands are available before repository commands.
- In this repository, command source files live in `packages/agents/commands/<command>.md`.
- `scripts/sync.py` generates Codex command artifacts from the shared command source.

IF a built-in slash command already does the Task:
### Use the built-in slash command
Do not write a custom Command that repeats Claude Code behavior.
Example: use `/hooks` to manage Hook wiring instead of writing a new command that lists Hooks.
Never: duplicate `/review`, `/model`, `/permissions`, `/plugins`, or `/mcp` under a new local command name.

IF no built-in slash command does the Task and the Architect should fire it manually:
### Write `commands/<command>.md`
Use a Command for a manual side effect or a manual Process the Architect explicitly invokes.
Template:
  ````markdown
  ---
  description: <what the command does>
  allowed-tools: <tool allowlist when the command needs tools>
  ---

  # /<command-name>

  <Prompt body, or a deterministic `!` command when the command is only a side effect.>
  ````
Example:
  ````markdown
  ---
  description: Copy current Plan path to clipboard
  allowed-tools: Bash(claude-current-plan-path:*)
  ---

  ```!
  claude-current-plan-path --copy
  ```
  ````
Never: put a recurring Agent-invoked Process in `commands/`; write a Skill and let the Agent invoke the Skill.

## 2. Built-in slash command facts

- `/clear` clears conversation history.
- `/compact [instructions]` compresses Context and can focus on specific topics.
- `/rewind` rolls back Context and code state; `Esc+Esc` and `/undo` are aliases, and it can resume a conversation from before `/clear` was run (v2.1.191+).
- `/recap` summarizes what happened since the User was last in the session (v2.1.108+).
- `/resume <name>` resumes a session by name.
- `/rename` names the current session, and `claude -n <name>` names a session at startup.
- `/branch [name]` forks the current session with the same history up to the branch point; `/fork` is an alias, and `claude --resume <id> --fork-session` is the command-line equivalent.
- `/btw <question>` runs an ephemeral side query with full conversation visibility, no tools, and no history entry; Space, Enter, or Escape dismisses it.
- `/exit` ends the session.
- `/loop [interval] <prompt>` schedules a recurring Prompt in the current session; `/proactive` is an alias, and the default interval is ten minutes.
- `/schedule [description]` creates a cloud scheduled Task that survives restarts and runs when the machine is off; the minimum interval is one hour.
- `/batch <instruction>` researches the codebase, decomposes the work into 5-30 independent units, spawns one background Agent per unit, and has each open a pull request; it requires a git repository.
- `/loop` timing is cron-based with one-minute minimum granularity.
- `/loop` fires on interval regardless of whether the previous run completed.
- `/loop` sends each firing as a new message into the session.
- `/loop` is session-scoped and does not spawn fresh sessions.
- `/loop` queues its message when the Agent is mid-turn and processes it after the current turn ends.
- `/loop` auto-expires after three days and allows at most 50 Tasks per session.
- `/loop` jitter can be up to ten percent of the period late, capped at fifteen minutes.
- `CLAUDE_CODE_DISABLE_CRON=1` disables `/loop` timing.
- `/config` opens the settings panel.
- `/config key=value` sets a setting from the Prompt, and `/config --help` lists shorthand keys.
- `/model` switches the model.
- `/effort` sets model effort level; no args opens an interactive slider, and `xhigh` applies on supported models or falls back to `high`.
- `/theme` opens the theme picker.
- `Ctrl+T` toggles syntax highlighting.
- `/theme` supports named custom themes.
- `/keybindings` configures keyboard shortcuts.
- `/permissions` views or updates tool permissions.
- `/allowed-tools` configures tool permissions interactively.
- `/tui` switches the conversation renderer.
- `/tui fullscreen` enables flicker-free alt-screen rendering with lower memory, mouse support, and copy-on-select (v2.1.110+).
- `/focus` toggles focus view, showing the Prompt, one-line tool summary with edit diffstats, and final response only (v2.1.110+).
- `/scroll-speed` tunes mouse-wheel scroll speed with a live preview (v2.1.139+).
- Vim editing is no longer a `/vim` command; toggle it through `/config` and editor mode (v2.1.92+).
- `/context` visualizes Context usage.
- `/memory` edits `CLAUDE.md` memory files.
- `/init` initializes a project with `CLAUDE.md`.
- `/add-dir` adds working directories.
- `/cd` moves the session to a new working directory without breaking the Prompt cache (v2.1.169+).
- `/help` shows available commands.
- `/doctor` checks installation health and update channel.
- `/status` shows account and system status.
- `/usage` shows plan limits, rate status, and usage by category, including Skills, Subagents, plugins, and per Model Context Protocol server cost.
- `/cost` and `/stats` are typing shortcuts that open the relevant `/usage` tab (v2.1.118+).
- `/hooks` manages automation Hooks.
- `/mcp` manages Model Context Protocol servers.
- `/mcp enable <name>` enables a specific Model Context Protocol server.
- `/agents` creates, edits, and lists agents.
- `/skills` accesses Skills.
- `/reload-skills` re-scans Skill directories without restarting (v2.1.152+).
- `/reload-plugins` re-scans and updates plugins, auto-installing missing dependencies.
- `/plugins` manages plugins, and `/plugin list` lists installed plugins.
- `/plugins discover` discovers available plugins.
- `/login` switches Anthropic accounts.
- `/logout` signs out.
- `/review` requests code Review and uses the `/code-review medium` engine.
- `/code-review [effort]` reports correctness bugs at the chosen effort; `--comment` posts inline pull request comments and `--fix` applies findings.
- `/simplify` runs cleanup-only Review for reuse, simplification, efficiency, and altitude, then applies the fixes (v2.1.154+).
- `/ultrareview [pull request number]` runs cloud multi-Agent Review of the current branch or a pull request.
- `claude ultrareview` runs `/ultrareview` non-interactively from continuous integration.
- `/goal` sets a completion Condition, and Claude keeps working across turns until it is met (v2.1.139+).
- `/workflows` views and manages dynamic workflows; trigger one in-Prompt with the `ultracode` keyword (v2.1.154+, renamed from `workflow` in v2.1.160).
- `/plan` enters plan mode.
- `/todos` shows tracked todo items.
- `/tasks` opens the background Tasks dialog.
- `/pr_comments` shows pull request comments.
- `/sandbox` enables sandboxed bash.
- `/voice` toggles voice input mode, shows dictation language on enable, and warns if the `language` setting is unsupported for voice input.
- `Ctrl+O` enters transcript mode; `/` searches, and `n` or `N` steps through matches.
- `Ctrl+X Ctrl+E` opens the external editor and aliases `Ctrl+G`.
- `Ctrl+X Ctrl+K` stops all background agents.
- `/teleport` resumes the session at claude.ai/code for claude.ai subscribers.
- `/remote-control` and `/rc` enable remote control from claude.ai/code or the mobile app.
- `claude --rc` starts a session with remote control enabled.
- `claude remote-control` starts server mode.
- `/remote-env` configures remote sessions for claude.ai subscribers.
- `/terminal-setup` installs the Shift+Enter keybinding.
- `/install-github-app` installs the GitHub Actions integration for pull request Reviews.
- `/bug` reports bugs and sends the conversation to Anthropic.
- `/copy [N]` copies the last response, or the Nth-latest response when N is provided.
- `/feedback` generates a GitHub issue link.
- `/export [filename]` exports the conversation.
- `/less-permission-prompts` scans transcripts for common read-only Bash and Model Context Protocol calls and proposes an allowlist for `.claude/settings.json` (v2.1.111+).
- `/team-onboarding` generates a teammate ramp-up guide from local Claude Code usage (v2.1.101+).
- `/powerup` opens interactive lessons teaching Claude Code features with animated demos (v2.1.90+).

## 3. Command-line flag facts

- `--bare` sets `CLAUDE_CODE_SIMPLE=1` and starts minimal mode.
- `--bare` skips Hooks, Language Server Protocol, plugin sync, attribution, auto-memory, background prefetches, keychain reads, and `CLAUDE.md` auto-discovery.
- Skills still resolve through `/skill-name` under `--bare`.
- `--bare` auth is strictly `ANTHROPIC_API_KEY` or `apiKeyHelper` through `--settings`; OAuth and keychain auth are not read.
- Third-party providers use their own credentials under `--bare`.
- Use `--append-system-prompt`, `--mcp-config`, `--settings`, `--agents`, and `--plugin-dir` to load Context explicitly under `--bare`.
- `--safe-mode` and `CLAUDE_CODE_SAFE_MODE` start with customizations disabled for troubleshooting, including `CLAUDE.md`, Skills, plugins, Hooks, Model Context Protocol servers, Commands, agents, output styles, workflows, themes, and keybindings; admin policy settings still apply (v2.1.169+).
- `--exclude-dynamic-system-prompt-sections` moves per-machine sections out of the system Prompt into the first User message to improve cross-User Prompt-cache reuse (v2.1.98+).
- `--bg` and `--background` start the session as a background Agent and return immediately; manage it with `claude agents` (v2.1.154+).
- `--agent <name>` starts the session with a custom agent's system Prompt, tools, and model; see `writing-agents.md`.
- `--add-dir <dirs...>` adds working directories.
- Skills in added directories' `.claude/skills/` directories are auto-discovered.
- `CLAUDE.md` files from added directories do not load by default; set `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` to enable them.
- `--rc [name]` starts a session with remote control enabled and differs from `claude remote-control` server mode.
- `--fork-session` creates a new session identifier instead of appending to the original when used with `--resume` or `--continue`; it is the command-line equivalent of `/branch`.
- `--channels` allows Model Context Protocol servers to push messages into the session and is a research preview.
- `--chrome` enables Chrome extension integration for frontend work and requires v2.0.73+.
- `-w` and `--worktree [name]` create a new git worktree for the session.
- `claude agents` opens an Agent view listing every Claude Code session as running, blocked, or done, and can dispatch or attach to background sessions (v2.1.139+).
- `claude project purge [path]` deletes Claude Code state for a project, including transcripts, Tasks, file history, and config entry; it supports `--dry-run`, `-y`, and `--all` (v2.1.126+).
- `claude mcp login <name>` and `claude mcp logout <name>` authenticate Model Context Protocol servers from the command line without the interactive `/mcp` menu; `--no-browser` supports SSH (v2.1.186+).
- `claude ultrareview [target]` runs `/ultrareview` non-interactively from continuous integration, and `--json` returns raw output (v2.1.120+).
- `claude plugin ...` is covered in `plugins-marketplace.md`.

## 4. Custom command facts

- Skills automatically become slash commands.
- Project custom Skill commands can be created in `.claude/skills/<skill-name>/SKILL.md`.
- Personal custom Skill commands can be created in `~/.claude/skills/<skill-name>/SKILL.md`.
- The Skill directory name becomes the command name.
- `/help` shows built-in commands and custom Skill commands.
- The model can discover and invoke built-in slash commands such as `/init`, `/review`, and `/security-review` through the Skill tool (v2.1.108+).

### Use the directory name as the command
The slash command is derived from the Skill directory name.
Example: `review/SKILL.md` becomes `/review`.
Never: make the frontmatter `name` disagree with the directory name when the command name matters.
