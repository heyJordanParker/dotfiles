"""Contracts for running several quick hooks in one process."""

import json
import os
import subprocess
import sys

from conftest import PY_HOOKS

RUNNER = os.path.join(PY_HOOKS, "combine_hooks.py")


def run(members, event):
    return subprocess.run([sys.executable, RUNNER, *members], input=json.dumps(event),
                          capture_output=True, text=True, timeout=30)


def test_every_member_refusal_reaches_the_agent():
    """Pins a refusal lost when one runner answers for several guards."""
    event = {"hook_event_name": "PreToolUse", "tool_name": "Bash", "cwd": "/tmp",
             "tool_input": {"command": "ls && git stash"}}

    result = run(["block_composed_commands", "block_git_revert"], event)

    assert result.returncode == 2
    assert "block_composed_commands_agent" in result.stderr
    assert "block_git_revert_agent" in result.stderr


def test_a_crashing_member_leaves_the_others_deciding(tmp_path):
    """Pins one broken hook disabling every other check in its group."""
    event = {"hook_event_name": "PreToolUse", "tool_name": "Bash", "cwd": "/tmp",
             "tool_input": {"command": "git stash"}}

    result = run(["missing_module", "block_git_revert"], event)

    assert result.returncode == 2
    assert "block_git_revert_agent" in result.stderr
    assert "missing_module" in result.stderr
