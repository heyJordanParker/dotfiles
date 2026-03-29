# Capture and Profiling

Video recording, Chrome DevTools profiling, and tracing for debugging, documentation, and performance analysis.

## Video Recording

Capture browser automation as WebM video:

```bash
agent-browser record start ./demo.webm
# ... perform actions ...
agent-browser record stop

# Restart with new file (stops current + starts new)
agent-browser record restart ./take2.webm
```

### Recording with error handling

```bash
#!/bin/bash
set -e
cleanup() {
    agent-browser record stop 2>/dev/null || true
    agent-browser close 2>/dev/null || true
}
trap cleanup EXIT

agent-browser record start ./automation.webm
# ... automation steps ...
```

### Combined video + screenshots

Capture video and key frames simultaneously:

```bash
agent-browser record start ./flow.webm
agent-browser open https://example.com
agent-browser screenshot ./step1-homepage.png
agent-browser click @e1
agent-browser screenshot ./step2-after-click.png
agent-browser record stop
```

Add `agent-browser wait 500` between steps if the video needs to be human-viewable.

## Chrome DevTools Profiling

Capture performance profiles for analysis:

```bash
agent-browser profiler start
agent-browser open https://app.example.com
agent-browser wait --load networkidle
agent-browser profiler stop ./trace.json
```

### Custom trace categories

```bash
agent-browser profiler start --categories "devtools.timeline,v8.execute,blink.user_timing"
# ... actions to profile ...
agent-browser profiler stop ./trace.json
```

Default categories: `devtools.timeline`, `v8.execute`, `blink`, `blink.user_timing`, `latencyInfo`, `renderer.scheduler`, `toplevel`.

### Viewing profiles

Load the output JSON in:
- **Chrome DevTools**: Performance panel → Load profile
- **Perfetto UI**: https://ui.perfetto.dev/ — drag and drop
- **Trace Viewer**: `chrome://tracing`

**Limitations:** Chromium-only (no Firefox/WebKit). Trace data accumulates in memory (capped at 5M events) — stop promptly after the area of interest. 30-second timeout on stop.

## Playwright Tracing

```bash
agent-browser trace start
# ... actions to trace ...
agent-browser trace stop ./trace.zip
```

## HAR Recording

See [network-control.md](network-control.md) for HAR recording of network traffic.
