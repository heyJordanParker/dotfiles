"""Coverage for the session-state tamper guard after it was repointed from the
/tmp control file to the spine's <sessions>/<id>/state.json.

The guard blocks a hook-external Write or shell command that targets a session
state.json directly, so a subagent can't rewrite control state behind the
spine's back — while the session hooks themselves pass through the
CLAUDE_SESSION_HOOK bypass. Each case runs the hook as a subprocess and asserts
the exit code (2 = blocked, 0 = allowed).
"""

import json
import os
import subprocess

from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "protect_session_state.py")
STATE_PATH = os.path.expanduser("~/.claude/sessions/abc123/state.json")
SUBAGENT_STATE = os.path.expanduser("~/.claude/sessions/parent/subagents/agent-x/state.json")


def _run(payload, hook_bypass=False):
    env = dict(os.environ)
    env.pop("CLAUDE_SESSION_HOOK", None)
    if hook_bypass:
        env["CLAUDE_SESSION_HOOK"] = "true"
    return subprocess.run(
        ["python3", HOOK], input=json.dumps(payload), text=True,
        capture_output=True, env=env,
    ).returncode


def test_blocks_direct_write_to_state_json():
    assert _run({"tool_input": {"file_path": STATE_PATH}}) == 2


def test_blocks_subagent_state_json():
    assert _run({"tool_input": {"file_path": SUBAGENT_STATE}}) == 2


def test_blocks_shell_write_to_state_json():
    assert _run({"tool_input": {"command": "echo {} > %s" % STATE_PATH}}) == 2


def test_session_hook_bypass_allows():
    assert _run({"tool_input": {"file_path": STATE_PATH}}, hook_bypass=True) == 0


def test_unrelated_file_path_allowed():
    assert _run({"tool_input": {"file_path": "packages/agents/hooks/foo.py"}}) == 0


def test_unrelated_command_allowed():
    assert _run({"tool_input": {"command": "echo hello world"}}) == 0
