# Runtime Config

Persistent settings via config file, environment variables, and global options.

## Config File

Create `agent-browser.json` in project root or user home for persistent defaults:

```json
{
  "headed": true,
  "proxy": "http://localhost:8080",
  "profile": "./browser-data",
  "userAgent": "my-agent/1.0",
  "ignoreHttpsErrors": true
}
```

**Priority (lowest to highest):**
1. `~/.agent-browser/config.json` (user-level)
2. `./agent-browser.json` (project-level)
3. `AGENT_BROWSER_*` env vars
4. CLI flags

Load specific config: `agent-browser --config ./ci-config.json open example.com` (or `AGENT_BROWSER_CONFIG` env). Exits with error if file is missing/invalid.

All CLI flags map to camelCase config keys (e.g., `--executable-path` → `"executablePath"`). Boolean flags accept `true`/`false` values (e.g., `--headed false` overrides config). Extensions from user and project configs are merged, not replaced.

## Environment Variables

```bash
AGENT_BROWSER_SESSION="mysession"              # Default session name
AGENT_BROWSER_SESSION_NAME="myapp"             # Auto-save/restore state persistence
AGENT_BROWSER_EXECUTABLE_PATH="/path/chrome"   # Custom browser path
AGENT_BROWSER_EXTENSIONS="/ext1,/ext2"         # Comma-separated extension paths
AGENT_BROWSER_PROVIDER="browserbase"           # Cloud browser provider
AGENT_BROWSER_ENGINE="lightpanda"              # Browser engine (chrome, lightpanda)
AGENT_BROWSER_HEADED="1"                       # Show browser window
AGENT_BROWSER_COLOR_SCHEME="dark"              # Color scheme preference
AGENT_BROWSER_DOWNLOAD_PATH="./downloads"      # Default download directory
AGENT_BROWSER_SCREENSHOT_DIR="./shots"         # Default screenshot directory
AGENT_BROWSER_SCREENSHOT_FORMAT="jpeg"         # Screenshot format (png, jpeg)
AGENT_BROWSER_SCREENSHOT_QUALITY="80"          # JPEG quality (0-100)
AGENT_BROWSER_STREAM_PORT="9223"               # WebSocket streaming port
AGENT_BROWSER_IDLE_TIMEOUT_MS="60000"          # Auto-shutdown after inactivity
AGENT_BROWSER_DEFAULT_TIMEOUT="25000"          # Default action timeout (ms)
AGENT_BROWSER_ENCRYPTION_KEY="<64-char-hex>"   # AES-256-GCM state encryption
AGENT_BROWSER_STATE_EXPIRE_DAYS="30"           # Auto-delete old states
AGENT_BROWSER_CONTENT_BOUNDARIES="1"           # Wrap output in boundary markers
AGENT_BROWSER_MAX_OUTPUT="50000"               # Truncate output to N chars
AGENT_BROWSER_ALLOWED_DOMAINS="example.com"    # Restrict navigation domains
AGENT_BROWSER_ACTION_POLICY="./policy.json"    # Action policy file
AGENT_BROWSER_CONFIRM_ACTIONS="eval,download"  # Categories requiring confirmation
AGENT_BROWSER_ANNOTATE="1"                     # Annotated screenshots by default
AGENT_BROWSER_DEBUG="1"                        # Debug output
```

Standard proxy env vars also supported: `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`.

## Global CLI Options

```bash
--proxy <url>              # Proxy server
--proxy-bypass <hosts>     # Bypass proxy for hosts
--user-agent <ua>          # Custom User-Agent
--extension <path>         # Load browser extension (repeatable)
--args <args>              # Browser launch args (comma separated)
--executable-path <path>   # Custom browser binary
--ignore-https-errors      # Ignore HTTPS cert errors
--allow-file-access        # Allow file:// URLs (Chromium only)
--state <path>             # Load storage state from JSON
--color-scheme <scheme>    # dark, light, no-preference
--download-path <path>     # Default download directory
--session-name <name>      # Auto-save/restore cookies + localStorage
--auto-connect             # Auto-discover running Chrome
--debug                    # Debug output
```

## Browser Settings

Runtime settings that persist for the session:

```bash
agent-browser set viewport 1920 1080         # Set viewport size
agent-browser set viewport 1920 1080 2       # 2x retina (same CSS size, higher res)
agent-browser set device "iPhone 14"         # Emulate device (viewport + user agent)
agent-browser set geo 37.7749 -122.4194      # Set geolocation
agent-browser set offline on                 # Toggle offline mode
agent-browser set headers '{"X-Key":"v"}'    # Extra HTTP headers
agent-browser set credentials user pass      # HTTP basic auth
agent-browser set media dark                 # Emulate color scheme
agent-browser set media light reduced-motion # Light mode + reduced motion
```

## Local Files

```bash
agent-browser --allow-file-access open file:///path/to/document.pdf
agent-browser --allow-file-access open file:///path/to/page.html
```
