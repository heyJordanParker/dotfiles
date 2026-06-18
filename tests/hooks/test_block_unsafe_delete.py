"""Behavior of the rm guard, plus a cross-check that its plugin copy agrees.

block_unsafe_delete.py is the source; block-unsafe-delete.sh is its
plugin-distributed shell copy. Both must reach the identical allow/block
decision on every command. Each case pins the expected exit code and asserts
both produce it, so the copy can never drift from the source. 0 = allow,
2 = block.

Both implementations resolve their whitelist to the repo root, so a path under
the repo or /tmp is allowed and /etc or the home root is not. Tests run from the
real repo, so REPO is the live whitelist root.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS, REPO

PY = os.path.join(PY_HOOKS, "block_unsafe_delete.py")
SH = os.path.join(REPO, "packages", "claude", "hooks", "block-unsafe-delete.sh")

ALLOW, BLOCK = 0, 2

# (command, cwd, expected_exit)
CASES = [
    # The circumvention from the brief: qualified rm path + unexpanded variable.
    ('/bin/rm "$HOME/newfile.txt"', REPO, BLOCK),
    # Qualified rm path, literal target outside the whitelist.
    ("/usr/bin/rm /etc/passwd", REPO, BLOCK),
    # Backslash alias-bypass, literal target outside the whitelist.
    (r"\rm /etc/passwd", REPO, BLOCK),
    # Backslash alias-bypass inside a command substitution.
    (r"$(\rm /etc/x)", REPO, BLOCK),
    # Bare rm, unexpanded variable target — unresolvable.
    ('rm "$path"', REPO, BLOCK),
    ("rm -rf $HOME/.ssh", REPO, BLOCK),
    # Piped rm — targets come from stdin, blocked regardless of cwd.
    ("find . -name x | xargs rm", REPO, BLOCK),
    ("find . -name x | xargs rm", "/etc", BLOCK),
    # rm inside a command substitution.
    ("$(rm /etc/x)", REPO, BLOCK),
    ("`rm /etc/x`", REPO, BLOCK),
    # Literal path outside the whitelist.
    ("rm /etc/passwd", REPO, BLOCK),
    # Glob with cwd outside the whitelist.
    ("rm *.log", "/etc", BLOCK),

    # Literal paths inside allowed directories.
    ("rm /tmp/foo", REPO, ALLOW),
    ("rm foo.txt", REPO, ALLOW),
    # Backslash alias-bypass with a literal target inside the whitelist.
    (r"\rm foo.txt", REPO, ALLOW),
    ("rm -rf build", REPO, ALLOW),
    # Glob with cwd inside the whitelist.
    ("rm *.log", REPO, ALLOW),
    # No rm at all.
    ("ls -la", "/etc", ALLOW),
    ("echo done", REPO, ALLOW),
    # 'perform' is not rm — the word boundary must not match it.
    ("git commit -m 'perform cleanup'", REPO, ALLOW),
    # rm as a non-command argument, literal target inside the whitelist.
    ("echo rm foo.txt", REPO, ALLOW),
]


def _payload(cmd, cwd):
    return json.dumps({"tool_input": {"command": cmd}, "cwd": cwd})


def _exit(runner, cmd, cwd):
    r = subprocess.run(runner, input=_payload(cmd, cwd), text=True, capture_output=True)
    return r.returncode


@pytest.mark.parametrize("cmd,cwd,expected", CASES, ids=[c[0] for c in CASES])
def test_python_and_shell_agree(cmd, cwd, expected):
    py = _exit(["python3", PY], cmd, cwd)
    sh = _exit(["bash", SH], cmd, cwd)
    assert py == expected, f"python: {py}, expected {expected} for {cmd!r}"
    assert sh == expected, f"shell: {sh}, expected {expected} for {cmd!r}"
