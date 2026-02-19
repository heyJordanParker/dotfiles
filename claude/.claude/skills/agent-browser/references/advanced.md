# Advanced Features

## Authentication via Headers

Skip login flows by injecting auth headers (scoped to origin, not leaked to other domains):

```bash
agent-browser open api.example.com --headers '{"Authorization": "Bearer <token>"}'

# Global headers (all domains)
agent-browser set headers '{"X-Custom-Header": "value"}'
```

## Persistent Profiles

Persist cookies, localStorage, IndexedDB, and login sessions across browser restarts:

```bash
agent-browser --profile ~/.myapp-profile open myapp.com
# Login once, then reuse authenticated session on next launch
agent-browser --profile ~/.myapp-profile open myapp.com/dashboard
```

Or via `AGENT_BROWSER_PROFILE` env var.

## Session Persistence

Auto-save/restore cookies and localStorage by session name:

```bash
agent-browser --session-name twitter open twitter.com
# State persists in ~/.agent-browser/sessions/
```

Encrypt session data with AES-256-GCM:
```bash
export AGENT_BROWSER_ENCRYPTION_KEY=<64-char-hex-key>  # openssl rand -hex 32
agent-browser --session-name secure open example.com
```

## CDP Mode (Connect to Existing Browser)

Connect to a running Chrome/Electron/WebView2 via Chrome DevTools Protocol:

```bash
# Connect once, then run commands without --cdp
agent-browser connect 9222
agent-browser snapshot

# Or per-command
agent-browser --cdp 9222 snapshot

# Remote browser via WebSocket
agent-browser --cdp "wss://browser-service.com/cdp?token=..." snapshot

# Auto-discover running Chrome (reads DevToolsActivePort, probes 9222/9229)
agent-browser --auto-connect snapshot
```

## iOS Simulator

Control real Mobile Safari. Requires macOS with Xcode.

**Setup:**
```bash
npm install -g appium && appium driver install xcuitest
```

**Usage:**
```bash
agent-browser device list                                    # List simulators
agent-browser -p ios --device "iPhone 16 Pro" open https://example.com
agent-browser -p ios snapshot -i
agent-browser -p ios tap @e1
agent-browser -p ios fill @e2 "text"
agent-browser -p ios swipe up
agent-browser -p ios swipe down 500
agent-browser -p ios screenshot mobile.png
agent-browser -p ios close
```

Or set `AGENT_BROWSER_PROVIDER=ios` and `AGENT_BROWSER_IOS_DEVICE="iPhone 16 Pro"`.

Real devices also supported via USB (requires signing WebDriverAgent in Xcode once).

## Cloud Providers

Use `-p <provider>` or `AGENT_BROWSER_PROVIDER` env var. All commands work identically.

- **Browserbase:** `BROWSERBASE_API_KEY` + `BROWSERBASE_PROJECT_ID` → `-p browserbase`
- **Browser Use:** `BROWSER_USE_API_KEY` → `-p browseruse`
- **Kernel:** `KERNEL_API_KEY` → `-p kernel` (supports stealth mode, persistent profiles via `KERNEL_PROFILE_NAME`)

## Local Files

```bash
agent-browser --allow-file-access open file:///path/to/document.pdf
agent-browser --allow-file-access open file:///path/to/page.html
```

## Streaming (Live Preview)

Stream browser viewport via WebSocket for live preview or pair browsing:

```bash
AGENT_BROWSER_STREAM_PORT=9223 agent-browser open example.com
# Connect to ws://localhost:9223 for JPEG frames + mouse/keyboard/touch input
```

## Configuration File

Set persistent defaults in `agent-browser.json` instead of repeating flags.

**Priority (lowest to highest):**
1. `~/.agent-browser/config.json` (user-level)
2. `./agent-browser.json` (project-level)
3. `AGENT_BROWSER_*` env vars
4. CLI flags

```json
{
  "headed": true,
  "proxy": "http://localhost:8080",
  "profile": "./browser-data",
  "userAgent": "my-agent/1.0",
  "ignoreHttpsErrors": true
}
```

Load specific config: `agent-browser --config ./ci-config.json open example.com`

All CLI flags map to camelCase config keys (e.g., `--executable-path` → `"executablePath"`).

## Browser Settings

```bash
agent-browser set viewport 1920 1080         # Set viewport size
agent-browser set device "iPhone 14"         # Emulate device
agent-browser set geo 37.7749 -122.4194      # Set geolocation
agent-browser set offline on                 # Toggle offline mode
agent-browser set credentials user pass      # HTTP basic auth
agent-browser set media dark                 # Emulate color scheme
agent-browser set media light reduced-motion # Light mode + reduced motion
```

## Cookies and Storage

```bash
agent-browser cookies                     # Get all cookies
agent-browser cookies set name value      # Set cookie
agent-browser cookies clear               # Clear cookies
agent-browser storage local               # Get all localStorage
agent-browser storage local key           # Get specific key
agent-browser storage local set k v       # Set value
agent-browser storage local clear         # Clear all
agent-browser storage session             # Same for sessionStorage
```

## Network Interception

```bash
agent-browser network route <url>                           # Intercept requests
agent-browser network route <url> --abort                   # Block requests
agent-browser network route <url> --body '{}' --status 200  # Mock response
agent-browser network unroute [url]                         # Remove routes
agent-browser network requests                              # View tracked requests
agent-browser network requests --filter api                 # Filter requests
```

## Frames (iframes)

```bash
agent-browser frame "#iframe"   # Switch to iframe (by selector, name, or url)
agent-browser frame main        # Back to main frame
```

## Video Recording

```bash
agent-browser record start ./demo.webm      # Start recording
agent-browser record stop                    # Stop and save
agent-browser record restart ./take2.webm    # Stop current + start new
```

## Low-Level Mouse and Keyboard

```bash
agent-browser mouse move 100 200    # Move mouse
agent-browser mouse down left       # Press button
agent-browser mouse up left         # Release button
agent-browser mouse wheel 100       # Scroll wheel
agent-browser keydown Shift          # Hold key down
agent-browser keyup Shift            # Release key
```

## Debugging and Profiling

```bash
agent-browser trace start              # Start Playwright trace
agent-browser trace stop trace.zip     # Stop and save trace
agent-browser profiler start           # Start Chrome DevTools profiling
agent-browser profiler stop trace.json # Stop and save profile
agent-browser har start                # Start HAR recording
agent-browser har stop output.har      # Stop and save HAR file
```

## Semantic Locators (full list)

```bash
agent-browser find role button click --name "Submit"
agent-browser find text "Sign In" click [--exact]
agent-browser find label "Email" fill "user@test.com"
agent-browser find placeholder "Search" type "query"
agent-browser find alt "Logo" click
agent-browser find title "Close" click
agent-browser find testid "submit-btn" click
agent-browser find first ".item" click
agent-browser find last ".item" click
agent-browser find nth 2 "a" hover
```

## Other Options

```bash
--proxy <url>              # Proxy server (AGENT_BROWSER_PROXY)
--proxy-bypass <hosts>     # Bypass proxy for hosts (AGENT_BROWSER_PROXY_BYPASS)
--user-agent <ua>          # Custom User-Agent (AGENT_BROWSER_USER_AGENT)
--extension <path>         # Load browser extension (repeatable; AGENT_BROWSER_EXTENSIONS)
--args <args>              # Browser launch args (AGENT_BROWSER_ARGS)
--executable-path <path>   # Custom browser binary (AGENT_BROWSER_EXECUTABLE_PATH)
--ignore-https-errors      # Ignore HTTPS cert errors
--state <path>             # Load storage state from JSON (AGENT_BROWSER_STATE)
--debug                    # Debug output
```
