"""Behavior for the browser-driving guard.

block_invalid_browsers.py blocks hand-driven browsers — Playwright, Puppeteer, the
raw Chrome binary, a remote-debugging attach, Selenium, Cypress, the drivers, and
agent-browser's own --headed — in executed command and script-payload position.
Automated test suites, process management, inspection, and --headed=false pass.
0 = allow, 2 = block.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

PY = os.path.join(PY_HOOKS, "block_invalid_browsers.py")

ALLOW, BLOCK = 0, 2

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

# (command, expected_exit)
CASES = [
    # Playwright drivers in a script payload.
    ("""node -e "const {chromium} = require('playwright'); await chromium.launch()" """, BLOCK),
    ("""node -e "await chromium.launchPersistentContext('/tmp/p', {})" """, BLOCK),
    ("""node -e "await chromium.connectOverCDP('http://localhost:9222')" """, BLOCK),
    ("""node -e "await chromium.launch({channel:'chrome'})" """, BLOCK),
    ("""node -e "require('playwright-core')" """, BLOCK),
    ("""node -e "require('@playwright/test')" """, BLOCK),
    ("""node -e "await firefox.launch()" """, BLOCK),
    ("""node -e "await webkit.launch()" """, BLOCK),

    # Puppeteer.
    ("""node -e "const puppeteer = require('puppeteer'); await puppeteer.launch()" """, BLOCK),
    ("""node -e "import puppeteer from 'puppeteer'" """, BLOCK),

    # The Chrome binary: directly, through a shell variable, and through spawn().
    ('"%s" --headless https://example.com' % CHROME, BLOCK),
    ('CHROME="%s" && "$CHROME" --headless' % CHROME, BLOCK),
    ("""node -e "spawn('%s', ['--headless'])" """ % CHROME, BLOCK),

    # Raw Chrome flags and the remote-debugging attach.
    ("chrome --remote-debugging-port=9222", BLOCK),
    ("chrome --dump-dom https://example.com", BLOCK),
    ("chrome --screenshot=/tmp/shot.png https://example.com", BLOCK),
    ("open -a 'Google Chrome' --args --remote-debugging-port=9222", BLOCK),

    # Other browser stacks.
    ("python3 -c 'from selenium import webdriver'", BLOCK),
    ("npx cypress open", BLOCK),
    ("chromedriver --port=9515", BLOCK),
    ("geckodriver --port=4444", BLOCK),
    ("python3 -c 'import pyppeteer'", BLOCK),
    ("python3 -c 'from playwright.sync_api import sync_playwright'", BLOCK),
    ("python3 -c 'from playwright.async_api import async_playwright'", BLOCK),

    # Launchers reached through a package runner, and the browser binary itself.
    ("npx playwright open https://example.com", BLOCK),
    ("npx playwright codegen https://example.com", BLOCK),
    ("npx playwright screenshot https://example.com shot.png", BLOCK),
    ("python3 -m playwright open https://example.com", BLOCK),
    ("npm exec playwright open", BLOCK),
    ("deno run npm:puppeteer", BLOCK),
    ("chromium https://example.com", BLOCK),
    ("google-chrome https://example.com", BLOCK),
    ("/Applications/Firefox.app/Contents/MacOS/firefox", BLOCK),

    # Opening a browser application: by name, by bundle, by path, and the default one.
    ("open -a 'Google Chrome' https://example.com", BLOCK),
    ("open -a 'Google Chrome.app'", BLOCK),
    ("open -a /Applications/Google\\ Chrome.app", BLOCK),
    ("open -b com.google.Chrome", BLOCK),
    ("open https://example.com", BLOCK),
    ("open -a Chromium https://example.com", BLOCK),
    ("open -a Safari https://example.com", BLOCK),
    ("open -a Firefox https://example.com", BLOCK),

    # agent-browser headed.
    ("agent-browser open https://example.com --headed", BLOCK),
    ("agent-browser open https://example.com --headed true", BLOCK),
    ("agent-browser open https://example.com --headed=true", BLOCK),
    ("agent-browser open https://example.com --headed 0", BLOCK),
    ("bunx agent-browser open https://example.com --headed", BLOCK),
    ("AGENT_BROWSER_HEADED=true agent-browser open https://example.com", BLOCK),
    ("agent-browser --config /tmp/browser.json open https://example.com", BLOCK),

    # agent-browser used correctly.
    ("agent-browser open https://example.com --headed=false", ALLOW),
    ("agent-browser open https://example.com --headed false", ALLOW),
    ("agent-browser snapshot", ALLOW),

    # Automated test suites and the wrappers resolving to them.
    ("playwright test", ALLOW),
    ("bun run test e2e", ALLOW),
    ("bun test e2e", ALLOW),
    ("bunx playwright test", ALLOW),
    ("npx playwright test", ALLOW),
    ("node_modules/.bin/playwright test", ALLOW),
    ("bun run test:e2e", ALLOW),
    ("npm run test:e2e", ALLOW),
    ("npx cypress run", ALLOW),
    ("uv run pytest -k selenium", ALLOW),

    # The test exemption is head-anchored: a launch payload with trailing
    # `playwright test` tokens is still a launch.
    ("""node -e 'require("playwright").chromium.launch()' playwright test""", BLOCK),

    # A launch inside command substitution runs whatever the outer head is.
    ("grep -r foo `chromium --headless https://example.com`", BLOCK),

    # Text that only names a browser.
    ("rg -n 'chromium.launch' src/", ALLOW),
    ("git grep -n webdriver", ALLOW),
    ("git commit -m 'remove cypress'", ALLOW),
    ("echo 'selenium is deprecated' >> NOTES.md", ALLOW),
    ("npm install --save-dev cypress", ALLOW),
    ("cat > f.js <<'EOF'\nconst b = await chromium.launch()\nEOF", ALLOW),

    # Process management and inspection.
    ("lsof -c chromedriver", ALLOW),
    ("killall chromedriver", ALLOW),
    ("which chromedriver", ALLOW),
    ('pkill -f "playwright test"', ALLOW),
    ('pgrep -f "Google Chrome"', ALLOW),
    ('ls -la "%s"' % CHROME, ALLOW),
    ('grep -r "chromium.launch" src/', ALLOW),
    ("sed -i '' 's/chromium.launch/agentBrowser.open/' src/run.js", ALLOW),
    ("trace grep chromium.launch", ALLOW),

    # Shell grammar and transparent prefixes in front of the launcher.
    ("if true; then chromium https://example.com; fi", BLOCK),
    ("{ chromium https://example.com; }", BLOCK),
    ("for x in 1; do chromium https://example.com; done", BLOCK),
    ("command chromium https://example.com", BLOCK),
    ("exec chromium https://example.com", BLOCK),
    ("nohup chromium https://example.com", BLOCK),
    ("sudo chromium https://example.com", BLOCK),
    ("time chromium https://example.com", BLOCK),
    ("setsid chromium https://example.com", BLOCK),
    ("caffeinate chromium https://example.com", BLOCK),
    ("stdbuf -o0 chromium https://example.com", BLOCK),
    ("timeout 60 chromium https://example.com", BLOCK),
    ("sudo npx playwright open", BLOCK),
    ("echo url | xargs open -a Safari", BLOCK),
    ("cat <(chromium --headless https://example.com)", BLOCK),

    # A command that cannot be parsed but names a browser fails closed.
    ('npx playwright open "https://example.com', BLOCK),

    # Runner spellings: version tags, `--`, command-string flags, `--eval=`, stdin.
    ("npx playwright@latest open", BLOCK),
    ("npm exec -- playwright@latest open", BLOCK),
    ('npm exec --call "playwright open https://example.com"', BLOCK),
    ('npx -c "playwright open https://example.com"', BLOCK),
    ("node --eval='puppeteer.launch()'", BLOCK),
    ("python3 - <<'PY'\nfrom selenium import webdriver\nPY", BLOCK),
    ("uv run --project /tmp python3 -c 'from selenium import webdriver'", BLOCK),

    # `open` reaches a browser bundle whatever the flag form.
    ("open /Applications/Google\\ Chrome.app", BLOCK),
    ("open -n -a 'Google Chrome'", BLOCK),
    ("open -a Arc", BLOCK),
    ('open -a "Google Chrome Canary"', BLOCK),

    # The config env var can turn headed on exactly like --config.
    ("AGENT_BROWSER_CONFIG=/path agent-browser open https://example.com", BLOCK),

    # An env assignment value is data, not a command; headed only matters on an open.
    ("BROWSER=chrome npm run test:e2e", ALLOW),
    ("BROWSER=chromium bun run test:e2e", ALLOW),
    ("ENGINE=chromium bun test", ALLOW),
    ("TARGET=safari make build", ALLOW),

    # A browser path in a variable is a launch only where the variable is executed.
    ('CHROME_BIN="%s" bun run test:e2e' % CHROME, ALLOW),
    ('CHROME_BIN="%s" npx playwright test' % CHROME, ALLOW),
    ('CHROME="%s" && echo "$CHROME"' % CHROME, ALLOW),
    ("AGENT_BROWSER_HEADED=true agent-browser close", ALLOW),
    ("AGENT_BROWSER_HEADED=true agent-browser open https://example.com", BLOCK),
    ("AGENT_BROWSER_HEADED=false agent-browser open https://example.com", ALLOW),

    # `open` on a file whose name happens to be a browser's short name.
    ("open dist/min", ALLOW),
    ("open -a Preview ~/docs/edge", ALLOW),

    # A browser reporting itself opens nothing, and a payload naming one drives nothing.
    ("google-chrome --version", ALLOW),
    ("chromium --help", ALLOW),
    ("chromedriver --version", ALLOW),
    ("""node -e "console.log('selenium')" """, ALLOW),

    # Substitution bodies are commands of their own, nested and resolved.
    ("""echo "$(node -e 'require("playwright").chromium.launch()')" """, BLOCK),
    ("""echo "$($(which chromium) --headless https://example.com)" """, BLOCK),
    ('echo "\\$(chromium --headless https://example.com)"', ALLOW),
    ("cat <<EOF\n$(chromium --headless https://example.com)\nEOF", BLOCK),
    ("cat <<'EOF'\n$(chromium --headless https://example.com)\nEOF", ALLOW),

    # Plain non-browser commands.
    ("ls -la", ALLOW),
    ("git status", ALLOW),
    ("uv run pytest tests/hooks", ALLOW),
]


def _payload(cmd):
    return json.dumps({"tool_input": {"command": cmd}})


def _run(cmd):
    return subprocess.run(["python3", PY], input=_payload(cmd), text=True, capture_output=True)


@pytest.mark.parametrize("cmd,expected", CASES, ids=[c[0].strip()[:60] for c in CASES])
def test_blocks_hand_driven_browsers(cmd, expected):
    r = _run(cmd)
    assert r.returncode == expected, f"exit {r.returncode}, expected {expected} for {cmd!r}"
    if expected == BLOCK:
        assert "BLOCKED: browser driving goes through agent-browser, headless." in r.stderr
        assert "agent-browser skills get core" in r.stderr


# Codex sends the command as a list joined into `/bin/zsh -lc <command>`, so every
# segment head is the shell until the wrapper is unwrapped.
CODEX_CASES = [
    (["/bin/zsh", "-lc", "agent-browser open https://example.com --headed"], BLOCK),
    (["/bin/zsh", "-lc", "npx playwright open https://example.com"], BLOCK),
    (["/bin/zsh", "-lc", "trace grep chromium.launch"], ALLOW),
    (["/bin/zsh", "-lc", "git commit -m 'remove cypress'"], ALLOW),
    (["/bin/zsh", "-lc", "playwright test"], ALLOW),
]


@pytest.mark.parametrize("cmd,expected", CODEX_CASES, ids=[c[0][2][:60] for c in CODEX_CASES])
def test_classifies_the_codex_shell_wrapper(cmd, expected):
    assert _run(cmd).returncode == expected
