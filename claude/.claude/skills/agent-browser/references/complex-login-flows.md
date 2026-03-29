# Complex Login Flows

OAuth, SSO, 2FA, token refresh, and importing auth from existing browsers. For basic auth (auth vault, profiles, session-name), see the main skill.

## OAuth / SSO Flows

Handle multi-step redirects:

```bash
agent-browser open https://app.example.com/auth/google
agent-browser wait --url "**/accounts.google.com**"
agent-browser snapshot -i

agent-browser fill @e1 "user@gmail.com"
agent-browser click @e2  # Next
agent-browser wait 2000
agent-browser snapshot -i
agent-browser fill @e3 "password"
agent-browser click @e4  # Sign in

agent-browser wait --url "**/app.example.com**"
agent-browser state save ./oauth-state.json
```

## Two-Factor Authentication

Use headed mode for manual 2FA completion:

```bash
agent-browser open https://app.example.com/login --headed
agent-browser snapshot -i
agent-browser fill @e1 "user@example.com"
agent-browser fill @e2 "password123"
agent-browser click @e3

# Wait for user to complete 2FA in the visible browser
agent-browser wait --url "**/dashboard"

# Save state so 2FA isn't needed again
agent-browser state save ./2fa-state.json
```

## Token Refresh / Session Expiry

Check if saved state is still valid before proceeding:

```bash
#!/bin/bash
STATE_FILE="./auth-state.json"

if [[ -f "$STATE_FILE" ]]; then
    agent-browser state load "$STATE_FILE"
    agent-browser open https://app.example.com/dashboard

    URL=$(agent-browser get url)
    if [[ "$URL" == *"/login"* ]]; then
        echo "Session expired, re-authenticating..."
        agent-browser snapshot -i
        agent-browser fill @e1 "$USERNAME"
        agent-browser fill @e2 "$PASSWORD"
        agent-browser click @e3
        agent-browser wait --url "**/dashboard"
        agent-browser state save "$STATE_FILE"
    fi
else
    agent-browser open https://app.example.com/login
    # ... first-time login flow ...
fi
```

## Import Auth from Running Chrome

Reuse cookies from a Chrome session you're already logged into:

```bash
# Step 1: User starts Chrome with remote debugging
# macOS: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --remote-debugging-port=9222
# Linux: google-chrome --remote-debugging-port=9222

# Step 2: Grab auth state
agent-browser --auto-connect state save ./my-auth.json

# Step 3: Use in automation
agent-browser --state ./my-auth.json open https://app.example.com/dashboard
```

Security: `--remote-debugging-port` exposes full browser control on localhost. Only use on trusted machines.

## HTTP Basic Auth

```bash
agent-browser set credentials username password
agent-browser open https://protected.example.com/api
```

## Cookie-Based Auth

```bash
agent-browser cookies set session_token "abc123xyz"
agent-browser open https://app.example.com/dashboard
```

## Auth Headers

Skip login flows by injecting auth headers (scoped to origin, not leaked to other domains):

```bash
agent-browser open api.example.com --headers '{"Authorization": "Bearer <token>"}'
```

## Security Notes

- Never commit state files — they contain session tokens. Add to `.gitignore`
- Use `--password-stdin` for auth vault to avoid shell history exposure
- Set `AGENT_BROWSER_ENCRYPTION_KEY` (64-char hex: `openssl rand -hex 32`) for encryption at rest
- Delete state files when no longer needed
- Use env vars for credentials: `agent-browser fill @e1 "$APP_USERNAME"`
