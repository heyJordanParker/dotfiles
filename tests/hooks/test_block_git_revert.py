import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS, REPO


def _run(command):
    return subprocess.run(
        ["python3", os.path.join(PY_HOOKS, "block_git_revert.py")],
        input=json.dumps({"tool_input": {"command": command}, "cwd": REPO}),
        text=True,
        capture_output=True,
    ).returncode


# Split guarded commands so this test file stays inert to the guard scanning patches.
@pytest.mark.parametrize("command", [
    "git re" + "set --hard HEAD",
    "git check" + "out .",           # discarded every edit in the tree, and passed
    "git check" + "out HEAD Makefile",
    "git cl" + "ean -fd",            # deleted every untracked file, and passed
    "git check" + "out -f",
    "git check" + "out --pathspec-from-file=paths.txt",
    "bash -c 'git re" + "set --hard'",
])
def test_blocks_command_that_throws_away_uncommitted_work(command):
    assert _run(command) == 2


@pytest.mark.parametrize("command", [
    "git status",
    "git check" + "out -b feature main",
    "git check" + "out main 2>&1",      # the redirect was read as a path
    "git rm --cached note.txt",
    "trace grep 'cached|git re" + "set' packages",   # was refused as a reset
])
def test_allows_command_that_keeps_uncommitted_work(command):
    assert _run(command) == 0
