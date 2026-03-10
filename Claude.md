# macOS Environment
v1.1 | Updated: 2026-03-09

## Why

Quick reference for system defaults that affect keybinding and development environment decisions.

## What

Reference documentation for terminal keybindings and local development services.

### Boundaries

- Never bind custom keybindings to the system defaults listed below

## How

### Terminal Keybindings

#### Ctrl (readline/shell)
- ^a - beginning of line
- ^e - end of line
- ^b - back one char
- ^f - forward one char
- ^d - delete forward
- ^h - delete backward
- ^k - kill to end of line
- ^u - kill whole line
- ^w - kill word backward
- ^y - yank (paste killed text)
- ^t - transpose chars
- ^p - previous history
- ^n - next history
- ^r - reverse search history
- ^l - clear screen
- ^c - interrupt
- ^z - suspend
- ^i - tab (same keycode)
- ^j - newline (same keycode)
- ^m - return (same keycode)

#### Alt/Option (word movement)
- ~b - back one word
- ~f - forward one word
- ~d - delete word forward
- ~Delete - delete word backward
- ~Enter - insert newline
- ~Tab - insert tab
- ~Esc - complete

#### Cmd - typically handled by terminal app, not shell
- Cmd+c - copy
- Cmd+v - paste
- Cmd+a - select all
- Cmd+. - cancel

### ~/Developer Directory

Two subdirectories with distinct purposes:

- **`references/`** — Temporary repos cloned for reading/studying code. Not synced or automated. Clone what you need, delete when done.
- **`services/`** — Repos we clone and run. Setup automated in `setup.sh` so any machine can reproduce.

#### Current Services

- **drawbridge** — Real-time diagram server for AI agents. Pushes simplified elements via HTTP → live Excalidraw canvas in browser.
  - Repo: `heyJordanParker/drawbridge`
  - Setup: `npm install && npm run build && npx playwright install chromium`
  - Run: `npm start` (API + WebSocket + static frontend on :3062)
  - Open: `http://localhost:3062/#session-name`
  - Skill: `/diagram` (installed at `~/.claude/skills/diagram/Skill.md`)

## Ledger

- v1.1: Adopted Why/What/How template
