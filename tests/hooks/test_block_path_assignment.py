"""Behavior for the zsh tied-parameter guard.

block_path_assignment.py blocks a bare assignment to a zsh tied search-path
parameter (path, cdpath, fpath, manpath), which would clobber the corresponding
array. A scoped temporary `path=x cmd` prefix, an untied name, the uppercase env
var, and an assignment that is really an argument are all left alone. 0 = allow,
2 = block.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

PY = os.path.join(PY_HOOKS, "block_path_assignment.py")

ALLOW, BLOCK = 0, 2

# (command, expected_exit)
CASES = [
    # Bare assignment to a tied parameter — clobbers the array.
    ("path=/foo/bar", BLOCK),
    ('path="$HOME/newfile.txt"', BLOCK),
    ("fpath=/x", BLOCK),
    ("cdpath=/x", BLOCK),
    ("manpath=/x", BLOCK),
    # A tied assignment among several bare assignments still clobbers.
    ("foo=1 path=/x", BLOCK),

    # Temporary prefix scoped to a command — left alone in zsh.
    ("path=/x ls", ALLOW),
    # Untied variable name.
    ("mypath=/x", ALLOW),
    # Uppercase env var is set the normal way, not the tied lowercase array.
    ("PATH=/x", ALLOW),
    # No assignment at all.
    ("ls -la", ALLOW),
    # 'path=' here is an argument to echo, not an assignment.
    ("echo path=/x", ALLOW),
]


def _payload(cmd):
    return json.dumps({"tool_input": {"command": cmd}})


def _exit(cmd):
    r = subprocess.run(["python3", PY], input=_payload(cmd), text=True, capture_output=True)
    return r.returncode


@pytest.mark.parametrize("cmd,expected", CASES, ids=[c[0] for c in CASES])
def test_blocks_tied_parameter_assignment(cmd, expected):
    rc = _exit(cmd)
    assert rc == expected, f"exit {rc}, expected {expected} for {cmd!r}"
