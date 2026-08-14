"""Behavior for the zsh tied-parameter guard.

block_path_assignment.py blocks a bare assignment to a zsh tied search-path
parameter (path, cdpath, fpath, manpath), which would clobber the corresponding
array. A scoped temporary `path=x cmd` prefix, an untied name, the uppercase env
var, and an assignment that is really an argument are all left alone. 0 = allow,
2 = block.
"""

import io
import json
import os
import subprocess
import sys

import block_path_assignment
import pytest
from conftest import PY_HOOKS

PY = os.path.join(PY_HOOKS, "block_path_assignment.py")

ALLOW, BLOCK = 0, 2

# (command, expected_exit)
CASES = [
    ("path=/foo/bar", BLOCK),
    ("path=/x ls", ALLOW),
]


# Every case calls main() in this process. These two stay a real `python3
# <hook>` run, one block and one allow, because the harness reads the exit code
# off the process rather than off a return value.
PROCESS = {"path=/foo/bar", "ls -la"}


def _payload(cmd):
    return json.dumps({"tool_input": {"command": cmd}})


def _exit(monkeypatch, cmd):
    if cmd in PROCESS:
        r = subprocess.run(["python3", PY], input=_payload(cmd), text=True,
                           capture_output=True)
        return r.returncode
    monkeypatch.setattr(sys, "stdin", io.StringIO(_payload(cmd)))
    return block_path_assignment.main()


@pytest.mark.parametrize("cmd,expected", CASES, ids=[c[0] for c in CASES])
def test_blocks_tied_parameter_assignment(monkeypatch, cmd, expected):
    rc = _exit(monkeypatch, cmd)
    assert rc == expected, f"exit {rc}, expected {expected} for {cmd!r}"
