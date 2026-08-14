"""Behavior for the browser-driving guard.

block_invalid_browsers.py blocks hand-driven browsers — Playwright, Puppeteer, the
raw Chrome binary, a remote-debugging attach, Selenium, Cypress, the drivers, and
agent-browser's own --headed — in executed command and script-payload position.
Automated test suites, process management, inspection, and --headed=false pass.
0 = allow, 2 = block.

The decision is `_launches_command`, a string in and a bool out, so the table calls
it directly. The two codex-wrapper cases below run the real process, because the
exit code and the stderr envelope are what the harness reads.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

PY = os.path.join(PY_HOOKS, "block_invalid_browsers.py")

ALLOW, BLOCK = 0, 2

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

# (command, expected_exit). One case per distinct decision the hook makes: a
# second spelling of a word already in one of its sets or regexes re-enters the
# same line and proves nothing the first spelling did not.




def _payload(cmd):
    return json.dumps({"tool_input": {"command": cmd}})


def _run(cmd):
    return subprocess.run(["python3", PY], input=_payload(cmd), text=True, capture_output=True)


# Codex sends the command as a list joined into `/bin/zsh -lc <command>`, so every
# segment head is the shell until the wrapper is unwrapped. These two run the real
# process: the exit code and the message on stderr are the harness's whole contract
# with this hook, and nothing above proves them.
CODEX_CASES = [
    (["/bin/zsh", "-lc", "agent-browser open https://example.com --headed"], BLOCK),
    (["/bin/zsh", "-lc", "trace grep chromium.launch"], ALLOW),
]


@pytest.mark.parametrize("cmd,expected", CODEX_CASES, ids=[c[0][2][:60] for c in CODEX_CASES])
def test_classifies_the_codex_shell_wrapper(cmd, expected):
    r = _run(cmd)
    assert r.returncode == expected
    if expected == BLOCK:
        assert "BLOCKED: browser driving goes through agent-browser, headless." in r.stderr
        assert "agent-browser skills get core" in r.stderr
