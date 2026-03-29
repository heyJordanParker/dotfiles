# Network Control

Intercept, mock, and proxy network traffic. Domain allowlists and action policies for security.

## Route / Mock / Block

```bash
agent-browser network route "**/api/*"                              # Intercept requests
agent-browser network route "**/api/*" --abort                      # Block requests
agent-browser network route "**/api/*" --body '{"ok":true}' --status 200  # Mock response
agent-browser network unroute                                       # Remove all routes
agent-browser network unroute "**/api/*"                            # Remove specific route
```

## Request Inspection

```bash
agent-browser network requests                           # View all tracked requests
agent-browser network requests --filter api              # Filter by URL pattern
agent-browser network requests --type xhr,fetch          # Filter by resource type
agent-browser network requests --method POST             # Filter by HTTP method
agent-browser network requests --status 2xx              # Filter by status (200, 2xx, 400-499)
agent-browser network requests --clear                   # Clear request log
agent-browser network request <requestId>                # Full request/response detail
```

## HAR Recording

```bash
agent-browser network har start                          # Start HAR recording
agent-browser network har stop ./capture.har             # Stop and save HAR file
```

## Proxy

```bash
# Via CLI flag
agent-browser --proxy "http://proxy.example.com:8080" open https://example.com

# Via env (standard proxy vars)
export HTTP_PROXY="http://proxy.example.com:8080"
export HTTPS_PROXY="http://proxy.example.com:8080"
agent-browser open https://example.com

# Authenticated proxy
agent-browser --proxy "http://user:pass@proxy.example.com:8080" open https://example.com

# SOCKS proxy
export ALL_PROXY="socks5://proxy.example.com:1080"
agent-browser open https://example.com

# Bypass proxy for specific hosts
agent-browser --proxy-bypass "localhost,*.internal.com" open https://example.com
# Or: export NO_PROXY="localhost,127.0.0.1,.company.com"
```

### Geo-testing with proxies

```bash
for region in us eu asia; do
    agent-browser --proxy "http://$region-proxy.example.com:8080" \
      --session "$region" open https://example.com
    agent-browser --session "$region" screenshot "./$region.png"
    agent-browser --session "$region" close
done
```

### Verify proxy connection

```bash
agent-browser open https://httpbin.org/ip
agent-browser get text body  # Should show proxy's IP
```

## Security Policies

### Domain allowlist

Restrict navigation to trusted domains. Wildcards match bare domains too (`*.example.com` matches `example.com`):

```bash
agent-browser --allowed-domains "example.com,*.example.com" open https://example.com
# Sub-resource requests and WebSocket connections to non-allowed domains are also blocked
```

### Action policy

Gate destructive actions via a JSON policy file:

```bash
AGENT_BROWSER_ACTION_POLICY=./policy.json agent-browser open https://example.com
```

Example `policy.json`:
```json
{ "default": "deny", "allow": ["navigate", "snapshot", "click", "scroll", "wait", "get"] }
```

Auth vault operations bypass action policy, but domain allowlist still applies.

### Content boundaries

Wrap page output in markers for LLM safety:

```bash
agent-browser --content-boundaries snapshot
# --- AGENT_BROWSER_PAGE_CONTENT nonce=<hex> origin=https://example.com ---
# [accessibility tree]
# --- END_AGENT_BROWSER_PAGE_CONTENT nonce=<hex> ---
```

### Output limits

Prevent context flooding from large pages:

```bash
agent-browser --max-output 50000 snapshot
```
