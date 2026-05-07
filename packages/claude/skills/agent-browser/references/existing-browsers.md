# Existing Browsers

Connect to and control browsers that are already running — Chrome via CDP, Electron apps (VS Code, Slack, Discord, Figma), and auto-discovery.

## CDP (Chrome DevTools Protocol)

Connect to a running Chrome/Chromium with remote debugging enabled:

```bash
# Connect once, then run commands without --cdp
agent-browser connect 9222
agent-browser snapshot -i
agent-browser click @e1

# Or per-command
agent-browser --cdp 9222 snapshot

# Remote browser via WebSocket
agent-browser --cdp "wss://browser-service.com/cdp?token=..." snapshot
```

## Auto-Connect

Auto-discover running Chrome without knowing the port:

```bash
agent-browser --auto-connect snapshot
agent-browser --auto-connect state save ./auth.json
```

Discovery order: `DevToolsActivePort` file → common ports (9222, 9229) → direct WebSocket fallback.

## Electron Apps

Electron apps (VS Code, Slack, Discord, Figma, Notion, Spotify) are built on Chromium and expose a CDP port. Launch with `--remote-debugging-port`, then use the standard snapshot-interact workflow:

```bash
# Launch the app with debugging enabled
# macOS example — Slack:
"/Applications/Slack.app/Contents/MacOS/Slack" --remote-debugging-port=9222

# Connect
agent-browser connect 9222
agent-browser snapshot -i

# Interact like any web page
agent-browser click @e1
agent-browser fill @e2 "message text"
```

Common Electron app paths (macOS):
- VS Code: `/Applications/Visual Studio Code.app/Contents/MacOS/Electron`
- Slack: `/Applications/Slack.app/Contents/MacOS/Slack`
- Discord: `/Applications/Discord.app/Contents/MacOS/Discord`
- Figma: `/Applications/Figma.app/Contents/MacOS/Figma`

## Tips

- Always `close` the session when done to avoid leaked daemon processes
- If a previous session wasn't closed properly, `agent-browser close --all` cleans up
- Use `agent-browser get cdp-url` to retrieve the WebSocket URL for external tools
