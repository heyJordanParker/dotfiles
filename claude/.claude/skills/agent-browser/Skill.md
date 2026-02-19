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
agent-browser scrollintoview @e1  # Scroll element into view
agent-browser drag @e1 @e2        # Drag and drop
agent-browser upload @e1 file.pdf # Upload file(s)
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
agent-browser is visible @e1      # Check visibility
agent-browser is enabled @e1      # Check if enabled
agent-browser is checked @e1      # Check if checked
```

### Screenshots and PDF
```bash
agent-browser screenshot               # Screenshot to stdout
agent-browser screenshot page.png      # Save to file
agent-browser screenshot --full        # Full page
agent-browser screenshot --annotate    # Numbered labels on interactive elements
agent-browser pdf output.pdf           # Save as PDF
```

The `--annotate` flag overlays `[N]` labels matching refs (`@eN`). After an annotated screenshot, interact using the same refs:
```bash
agent-browser screenshot --annotate ./page.png
agent-browser click @e2  # Click element labeled [2]
```

### Wait
```bash
agent-browser wait @e1                     # Wait for element
agent-browser wait 2000                    # Wait milliseconds
agent-browser wait --text "Success"        # Wait for text
agent-browser wait --load networkidle      # Wait for network idle
agent-browser wait --url "**/dashboard"    # Wait for URL pattern
agent-browser wait --fn "window.ready"    # Wait for JS condition
```

### Sessions (parallel browsers)
```bash
agent-browser --session test1 open site-a.com
agent-browser --session test2 open site-b.com
agent-browser session list
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
```

### Debugging
```bash
agent-browser open example.com --headed  # Show browser window
agent-browser highlight @e1              # Highlight element visually
agent-browser console                    # View console messages
agent-browser errors                     # View page errors
```

## References

- [advanced.md](references/advanced.md) — Persistence, authentication, CDP, iOS, cloud providers, configuration
