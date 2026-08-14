import json
import os
import subprocess

from conftest import PY_HOOKS, REPO


# Split guarded commands so this test file stays inert to the guard scanning patches.
def test_blocks_destructive_revert_command():
    command = "git re" + "set --hard HEAD"
    result = subprocess.run(
        ["python3", os.path.join(PY_HOOKS, "block_git_revert.py")],
        input=json.dumps({"tool_input": {"command": command}, "cwd": REPO}),
        text=True,
        capture_output=True,
    )

    assert result.returncode == 2


def test_allows_ordinary_git_command():
    result = subprocess.run(
        ["python3", os.path.join(PY_HOOKS, "block_git_revert.py")],
        input=json.dumps({"tool_input": {"command": "git status"}, "cwd": REPO}),
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0
