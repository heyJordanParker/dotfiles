# Remote Environments

iOS simulator, cloud browser providers, and alternative browser engines.

## iOS Simulator

Control real Mobile Safari. Requires macOS with Xcode.

**Setup:**
```bash
npm install -g appium && appium driver install xcuitest
```

**Usage:**
```bash
agent-browser -p ios device list                                    # List simulators
agent-browser -p ios --device "iPhone 16 Pro" open https://example.com
agent-browser -p ios snapshot -i
agent-browser -p ios tap @e1                                        # Touch element
agent-browser -p ios fill @e2 "text"
agent-browser -p ios swipe up                                       # Mobile gesture
agent-browser -p ios swipe down 500
agent-browser -p ios screenshot mobile.png
agent-browser -p ios close
```

Or set via env: `AGENT_BROWSER_PROVIDER=ios` and `AGENT_BROWSER_IOS_DEVICE="iPhone 16 Pro"`.

**Real devices:** Works with physical iOS devices via USB. Use `--device "<UDID>"` where UDID is from `xcrun xctrace list devices`. Requires signing WebDriverAgent in Xcode once.

## Cloud Providers

Use `-p <provider>` or `AGENT_BROWSER_PROVIDER` env. All commands work identically to local Chrome:

- **Browserbase:** `BROWSERBASE_API_KEY` + `BROWSERBASE_PROJECT_ID` → `-p browserbase`
- **Browser Use:** `BROWSER_USE_API_KEY` → `-p browseruse`
- **Kernel:** `KERNEL_API_KEY` → `-p kernel` (supports stealth mode, persistent profiles via `KERNEL_PROFILE_NAME`)
- **Browserless:** `-p browserless`

```bash
BROWSERBASE_API_KEY="..." BROWSERBASE_PROJECT_ID="..." \
  agent-browser -p browserbase open https://example.com
agent-browser snapshot -i  # Same workflow as local
```

## Lightpanda Engine

Alternative headless browser — 10x faster, 10x less memory than Chrome:

```bash
agent-browser --engine lightpanda open example.com
# Or via env
AGENT_BROWSER_ENGINE=lightpanda agent-browser open example.com
# Custom binary path
agent-browser --engine lightpanda --executable-path /path/to/lightpanda open example.com
```

**Limitations:** Does not support `--extension`, `--profile`, `--state`, or `--allow-file-access`.

Install from https://lightpanda.io/docs/open-source/installation.
