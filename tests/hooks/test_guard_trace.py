"""Behavior of the trace guard.

guard_trace blocks two shapes: a real `trace` command piped into a trimmer or
redirected into a repo file (block A), and a raw cat/grep/find/sed/etc. against
a path that exists in the repo (block B). Everything else passes. Both shapes
read the command through lib.command's quote-aware parser, so text inside a
quoted argument — a commit message, an echo string — is never command structure.
0 = allow, 2 = block. cwd is the live repo so in-repo paths resolve.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS, REPO

PY = os.path.join(PY_HOOKS, "guard_trace.py")
ALLOW, BLOCK = 0, 2

# (command, cwd, expected_exit)
CASES = [
    # --- block A: trace piped into a trimmer ---
    ("trace grep foo | head", REPO, BLOCK),
    ("trace status | tail -5", REPO, BLOCK),
    ("trace read x.py | grep handler", REPO, BLOCK),
    ("trace grep \"foo bar\" | wc -l", REPO, BLOCK),
    # trace inside a command substitution still counts.
    ("echo $(trace status | head)", REPO, BLOCK),

    # --- block A: trace redirected into a repo file ---
    ("trace read x.py > README.md", REPO, BLOCK),
    ("trace status >> README.md", REPO, BLOCK),

    # --- block B: raw read tool against a real repo path ---
    ("cat README.md", REPO, BLOCK),
    ("grep foo packages/agents/hooks/guard_trace.py", REPO, BLOCK),
    ("find . -name x", REPO, BLOCK),
    ("sed -n 1,5p README.md", REPO, BLOCK),

    # --- the regression: quoted text is not command structure ---
    # Commit message mentioning trace + a trailing pipe must not block.
    ('git commit -m "refactor: use trace status | tail for verify"', REPO, ALLOW),
    ('git commit -m "see the trace status output"', REPO, ALLOW),
    # A quoted trace mention piped into a trimmer is not a trace invocation.
    ('echo "trace status" | wc -l', REPO, ALLOW),

    # --- plain trace always passes ---
    ("trace status", REPO, ALLOW),
    ("trace grep foo", REPO, ALLOW),
    ("trace read packages/agents/hooks/guard_trace.py", REPO, ALLOW),
    # Stderr fd-dup is not a file redirect.
    ("trace read x.py 2>&1", REPO, ALLOW),
    # Redirect to a path outside the repo.
    ("trace status > /tmp/out.txt", REPO, ALLOW),
    # Semicolon is not a pipe.
    ("trace status; echo done", REPO, ALLOW),

    # --- non-matching commands pass ---
    ("ls -la", REPO, ALLOW),
    ("echo done", REPO, ALLOW),
    # Raw read tool against a path outside the repo.
    ("cat /etc/hosts", REPO, ALLOW),
]


def _payload(cmd, cwd):
    return json.dumps({"tool_input": {"command": cmd}, "cwd": cwd})


def _exit(cmd, cwd):
    r = subprocess.run(["python3", PY], input=_payload(cmd, cwd),
                       text=True, capture_output=True)
    return r.returncode


@pytest.mark.parametrize("cmd,cwd,expected", CASES, ids=[c[0] for c in CASES])
def test_guard_trace(cmd, cwd, expected):
    rc = _exit(cmd, cwd)
    assert rc == expected, f"exit {rc}, expected {expected} for {cmd!r}"
