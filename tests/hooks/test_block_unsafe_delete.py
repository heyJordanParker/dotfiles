"""Behavior of the rm guard, plus a cross-check that its plugin copy agrees.

block_unsafe_delete.py is the source; block-unsafe-delete.sh is its
plugin-distributed shell copy. Both must reach the identical allow/block
decision on every command. Each case pins the expected exit code and asserts
both produce it, so the copy can never drift from the source. 0 = allow,
2 = block.

The shell copy is a real `bash <script>` run on every case — that dual run is
the only thing that catches drift. The Python side calls main() in this process,
except for the two cases in PROCESS, one block and one allow, which stay a real
`python3 <hook>` run because the harness reads the exit code off the process.

Both implementations resolve their whitelist to the repo root, so a path under
the repo or /tmp is allowed and /etc or the home root is not. Tests run from the
real repo, so REPO is the live whitelist root.
"""

import io
import json
import os
import subprocess
import sys

import block_unsafe_delete
import pytest
from conftest import PY_HOOKS, REPO

PY = os.path.join(PY_HOOKS, "block_unsafe_delete.py")
SH = os.path.join(REPO, "packages", "claude", "hooks", "block-unsafe-delete.sh")

ALLOW, BLOCK = 0, 2

# (command, cwd, expected_exit)
CASES = [
    ('/bin/rm "$HOME/newfile.txt"', REPO, BLOCK),
    ("rm /tmp/foo", REPO, ALLOW),
]


PROCESS = {"rm /etc/passwd", "rm foo.txt"}


def _payload(cmd, cwd):
    return json.dumps({"tool_input": {"command": cmd}, "cwd": cwd})


def _exit(runner, cmd, cwd):
    r = subprocess.run(runner, input=_payload(cmd, cwd), text=True, capture_output=True)
    return r.returncode


def _python_exit(monkeypatch, cmd, cwd):
    if cmd in PROCESS:
        return _exit(["python3", PY], cmd, cwd)
    monkeypatch.setattr(sys, "stdin", io.StringIO(_payload(cmd, cwd)))
    return block_unsafe_delete.main()


@pytest.mark.parametrize("cmd,cwd,expected", CASES, ids=[c[0] for c in CASES])
def test_python_and_shell_agree(monkeypatch, cmd, cwd, expected):
    py = _python_exit(monkeypatch, cmd, cwd)
    sh = _exit(["bash", SH], cmd, cwd)
    assert py == expected, f"python: {py}, expected {expected} for {cmd!r}"
    assert sh == expected, f"shell: {sh}, expected {expected} for {cmd!r}"
