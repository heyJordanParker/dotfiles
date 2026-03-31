---
name: agent-browser
description: Automates browser interactions for web testing, form filling, screenshots, and data extraction. Use when the user needs to navigate websites, interact with web pages, fill forms, take screenshots, test web applications, or extract information from web pages.
---

# Browser Automation with agent-browser

## Core Workflow

1. `agent-browser open <url>` — navigate to page
2. `agent-browser snapshot -i` — get interactive elements with refs (`@e1`, `@e2`)
3. Interact using refs from snapshot
4. Re-snapshot after navigation or significant DOM changes

## Execution Rules

- **Always synchronous** — Run all `agent-browser` commands via Bash with default settings. Never use `run_in_background: true`. Commands execute sequentially — wait for each to complete before running the next
- **Always headless** — Never use `--headed` or set `headed: true` unless the user explicitly requests a visible browser. The browser must be invisible — no windows, no popups, no stealing focus

## Commands

### Navigation
```bash
agent-browser open <url>
agent-browser back
agent-browser forward
agent-browser reload
agent-browser close
```

### Snapshot (page analysis)
```bash
agent-browser snapshot           # Full accessibility tree
agent-browser snapshot -i        # Interactive elements only (recommended)
agent-browser snapshot -i -C     # Include cursor-interactive elements (onclick divs, etc.)
agent-browser snapshot -c        # Compact (remove empty structural elements)
agent-browser snapshot -d 3      # Limit depth to 3
agent-browser snapshot -s "#main"  # Scope to CSS selector
agent-browser snapshot -i -c -d 5  # Combine options
```

### Interactions (use @refs from snapshot)
```bash
agent-browser click @e1           # Click
agent-browser click @e1 --new-tab # Click and open in new tab
agent-browser dblclick @e1        # Double-click
agent-browser fill @e2 "text"     # Clear and type
agent-browser type @e2 "text"     # Type without clearing
agent-browser press Enter         # Press key
agent-browser press Control+a     # Key combination
agent-browser hover @e1           # Hover
agent-browser focus @e1           # Focus element
agent-browser check @e1           # Check checkbox
agent-browser uncheck @e1         # Uncheck checkbox
agent-browser select @e1 "value"  # Select dropdown
agent-browser scroll down 500     # Scroll page
agent-browser scroll down 500 --selector "div.content" # Scroll within container
agent-browser scrollintoview @e1  # Scroll element into view
agent-browser keyboard type "text" # Type at current focus (no selector)
agent-browser keyboard inserttext "text" # Insert text without key events
agent-browser drag @e1 @e2        # Drag and drop (element to element)
agent-browser drag "#draggable" "#drop-zone" # Drag by CSS selector
agent-browser upload @e1 file.pdf # Upload file(s)
agent-browser download @e1 ./file.pdf # Click element to trigger download
agent-browser keydown Shift       # Hold key down
agent-browser keyup Shift         # Release key
```

### Mouse (low-level control)

For canvas apps, maps, drawing tools, and precision drag operations where `drag` isn't enough:

```bash
agent-browser mouse move 100 200  # Move to coordinates
agent-browser mouse down          # Press left button (default)
agent-browser mouse down right    # Press right button
agent-browser mouse down middle   # Press middle button
agent-browser mouse up            # Release left button (default)
agent-browser mouse up right      # Release right button
agent-browser mouse wheel 100     # Scroll down (positive = down)
agent-browser mouse wheel -50     # Scroll up
agent-browser mouse wheel 0 100   # Scroll right (horizontal)
```

**Manual drag pattern** (for canvas/map/drawing where element refs don't apply):
```bash
agent-browser mouse move 200 300   # Move to start position
agent-browser mouse down           # Press
agent-browser mouse move 500 300   # Drag to end position
agent-browser mouse up             # Release
```

**Shift-drag / modifier-drag:**
```bash
agent-browser keydown Shift
agent-browser mouse move 200 300
agent-browser mouse down
agent-browser mouse move 500 300
agent-browser mouse up
agent-browser keyup Shift
```

### Selectors (alternatives to @refs)
```bash
agent-browser click "#id"          # CSS selector
agent-browser click ".class"       # CSS class
agent-browser click "text=Submit"  # Text content
agent-browser click "xpath=//button"  # XPath
agent-browser find role button click --name "Submit"  # Semantic locator
agent-browser find label "Email" fill "user@test.com"  # By label
```

### Get information
```bash
agent-browser get text @e1        # Get element text
agent-browser get html @e1        # Get innerHTML
agent-browser get value @e1       # Get input value
agent-browser get attr @e1 href   # Get attribute
agent-browser get title           # Get page title
agent-browser get url             # Get current URL
agent-browser get count ".item"   # Count matching elements
agent-browser get box @e1         # Get bounding box
agent-browser get styles @e1      # Get computed styles
agent-browser get cdp-url         # Get CDP WebSocket URL
agent-browser is visible @e1      # Check visibility
agent-browser is enabled @e1      # Check if enabled
agent-browser is checked @e1      # Check if checked
```

### Screenshots and PDF
```bash
agent-browser screenshot               # Screenshot to temp dir
agent-browser screenshot page.png      # Save to file
agent-browser screenshot @e1           # Screenshot specific element
agent-browser screenshot --full        # Full page
agent-browser screenshot --annotate    # Numbered labels on interactive elements
agent-browser screenshot --annotate ./page.png   # Save annotated screenshot
agent-browser screenshot --screenshot-dir ./shots # Save to custom directory
agent-browser screenshot --screenshot-format jpeg --screenshot-quality 80
agent-browser pdf output.pdf           # Save as PDF
```

The `--annotate` flag overlays `[N]` labels matching refs (`@eN`). Also caches refs — interact without a separate snapshot:
```bash
agent-browser screenshot --annotate
# [1] @e1 button "Submit"  [2] @e2 link "Home"  [3] @e3 textbox "Email"
agent-browser click @e2  # Click element labeled [2]
```

Full page capture (screenshot + text + PDF):
```bash
agent-browser open "$URL" && agent-browser wait --load networkidle
agent-browser screenshot --full ./page.png
agent-browser get text body > ./page.txt
agent-browser pdf ./page.pdf
```

### Wait
```bash
agent-browser wait @e1                     # Wait for element
agent-browser wait "#spinner" --state hidden  # Wait for element to disappear
agent-browser wait 2000                    # Wait milliseconds
agent-browser wait --text "Success"        # Wait for text (substring match)
agent-browser wait --load networkidle      # Wait for network idle
agent-browser wait --url "**/dashboard"    # Wait for URL pattern
agent-browser wait --fn "window.ready"     # Wait for JS condition
agent-browser wait --fn "!document.body.innerText.includes('Loading...')" # Wait for text to disappear
agent-browser wait --download [path]       # Wait for download to complete
```

### Sessions (parallel browsers)
```bash
agent-browser --session test1 open site-a.com
agent-browser --session test2 open site-b.com
agent-browser session                  # Show current session name
agent-browser session list             # List active sessions
```

### Command chaining

Commands can be chained with `&&`. The browser persists via a background daemon:
```bash
agent-browser open example.com && agent-browser wait --load networkidle && agent-browser snapshot -i
```

Use `&&` when you don't need intermediate output. Run commands separately when you need to parse output (e.g., snapshot to discover refs before interacting).

### JSON output
```bash
agent-browser snapshot -i --json
agent-browser get text @e1 --json
```

### Tabs
```bash
agent-browser tab                 # List tabs
agent-browser tab new [url]       # New tab
agent-browser tab 2               # Switch to tab 2
agent-browser tab close [n]       # Close tab
```

### Dialogs
```bash
agent-browser dialog accept [text]  # Accept dialog (optional prompt text)
agent-browser dialog dismiss        # Dismiss dialog
agent-browser dialog status         # Check if a dialog is currently open
```

### JavaScript
```bash
agent-browser eval 'document.title'          # Evaluate expression
agent-browser eval -b "<base64>"             # Base64-encoded script
agent-browser eval --stdin <<< 'return 1+1'  # Script from stdin
```

### State management
```bash
agent-browser state save auth.json    # Save cookies, storage, auth state
agent-browser state load auth.json    # Restore saved state
agent-browser state list              # List saved state files
agent-browser state show auth.json    # Show state summary
agent-browser state rename old new    # Rename state file
agent-browser state clear [name]      # Clear states for session name
agent-browser state clean --older-than 7 # Delete old states
```

### Debugging
```bash
agent-browser open example.com --headed  # Show browser window
agent-browser --color-scheme dark open example.com # Dark mode
agent-browser highlight @e1              # Highlight element visually
agent-browser inspect                    # Open Chrome DevTools
agent-browser console                    # View console messages
agent-browser errors                     # View page errors
agent-browser close                      # Close browser
agent-browser close --all               # Close all active sessions
```

### Authentication

```bash
# Auth vault — credentials stored encrypted, login by name (recommended)
echo "$PASSWORD" | agent-browser auth save myapp --url https://app.example.com/login --username user --password-stdin
agent-browser auth login myapp        # Login using saved credentials
agent-browser auth list               # List saved profiles
agent-browser auth show myapp         # Show profile metadata
agent-browser auth delete myapp       # Delete profile

# Persistent profile — reuse login across restarts (cookies, IndexedDB, cache)
agent-browser --profile ~/.myapp open https://app.example.com/login
# ... login once, then all future runs are authenticated:
agent-browser --profile ~/.myapp open https://app.example.com/dashboard

# Session-name persistence — auto-save/restore cookies + localStorage
agent-browser --session-name myapp open https://app.example.com/login
# ... login, then close. State auto-saved to ~/.agent-browser/sessions/
agent-browser --session-name myapp open https://app.example.com/dashboard  # Auto-restored

# Connect to existing Chrome — reuse its logged-in state
agent-browser --auto-connect state save ./auth.json
agent-browser --state ./auth.json open https://app.example.com/dashboard

# CDP — connect to a running browser
agent-browser connect 9222            # Connect once, then run commands
agent-browser --cdp 9222 snapshot     # Or per-command
```

`auth login` waits for login form selectors to appear before filling — more reliable on SPA login pages. For OAuth, SSO, 2FA, and token refresh patterns, see [complex-login-flows.md](references/complex-login-flows.md).

**Session expiry:** After loading state, verify you're still authenticated:
```bash
agent-browser --state ./auth.json open https://app.example.com/dashboard
URL=$(agent-browser get url)
# If redirected to login, session expired — re-authenticate
```

### Iframes

Iframe content is automatically inlined in snapshots. Refs inside iframes carry frame context — interact directly without switching frames:
```bash
agent-browser snapshot -i
# @e2 [Iframe] "payment-frame"
#   @e3 [input] "Card number"
agent-browser fill @e3 "4111111111111111" # Works directly

# Or scope to one iframe
agent-browser frame @e2               # Switch to iframe
agent-browser snapshot -i             # Only iframe content
agent-browser frame main              # Return to main frame
```

### Diff (verifying changes)
```bash
agent-browser diff snapshot                          # Compare current vs last snapshot
agent-browser diff screenshot --baseline before.png  # Visual pixel diff
agent-browser diff url <url1> <url2>                 # Compare two pages
```

### Batch execution
```bash
echo '[["open", "https://example.com"], ["snapshot", "-i"], ["screenshot", "result.png"]]' | agent-browser batch --json
agent-browser batch --bail < commands.json  # Stop on first error
```

### Streaming
```bash
agent-browser stream enable              # Start WebSocket streaming (auto port)
agent-browser stream enable --port 9223  # Specific port
agent-browser stream status              # Show streaming state
agent-browser stream disable             # Stop streaming
```

### Network inspection
```bash
agent-browser network requests                 # View tracked requests
agent-browser network requests --type xhr,fetch # Filter by resource type
agent-browser network requests --method POST   # Filter by HTTP method
agent-browser network requests --status 2xx    # Filter by status
agent-browser network request <requestId>      # Full request/response detail
agent-browser network har start                # Start HAR recording
agent-browser network har stop ./capture.har   # Stop and save HAR
```

### Clipboard
```bash
agent-browser clipboard read             # Read text from clipboard
agent-browser clipboard write "text"     # Write text to clipboard
agent-browser clipboard copy             # Copy current selection
agent-browser clipboard paste            # Paste from clipboard
```

### Security
```bash
agent-browser --content-boundaries open example.com  # Wrap output in boundary markers (recommended for AI)
agent-browser --allowed-domains "example.com,*.example.com" open example.com  # Domain allowlist
agent-browser --max-output 50000 snapshot            # Prevent context flooding
```

## References

- [complex-login-flows.md](references/complex-login-flows.md) — OAuth, SSO, 2FA, token refresh, importing auth from Chrome
- [existing-browsers.md](references/existing-browsers.md) — CDP, auto-connect, Electron apps
- [remote-environments.md](references/remote-environments.md) — iOS simulator, cloud providers, Lightpanda engine
- [network-control.md](references/network-control.md) — Route/mock, proxy, HAR, domain allowlist, action policy
- [capture-and-profiling.md](references/capture-and-profiling.md) — Video recording, DevTools profiling, tracing
- [runtime-config.md](references/runtime-config.md) — Config file, env vars, engine selection
