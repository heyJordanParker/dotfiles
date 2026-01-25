# User Commands

Built-in slash commands for Claude Code.

## Session & History

- **/clear** — Clear conversation history
- **/compact [instructions]** — Compress context, optionally focus on specific topics
- **/rewind** — Roll back context and code state (also `Esc+Esc`)
- **/resume <name>** — Resume session by name
- **/rename** — Name current session
- **/exit** — End session

## Configuration

- **/config** — Open settings panel (searchable)
- **/model** — Switch AI model
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

## Session Transfer

For claude.ai subscribers:

- **/teleport** — Resume session at claude.ai/code
- **/remote-env** — Configure remote sessions

## Setup & Integrations

- **/terminal-setup** — Install Shift+Enter keybinding
- **/install-github-app** — GitHub Actions integration for PR reviews

## Utilities

- **/bug** — Report bugs (sends conversation to Anthropic)
- **/feedback** — Generate GitHub issue URL
- **/export [filename]** — Export conversation
- **/tag** — Tag sessions

## Custom Skills as Commands

Skills automatically become slash commands. Create in:
- **Project:** `.claude/skills/skill-name/Skill.md`
- **Personal:** `~/.claude/skills/skill-name/Skill.md`

Directory name becomes command: `review/Skill.md` → `/review`

Run `/help` to see all commands including custom skills.
