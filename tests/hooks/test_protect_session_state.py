"""Coverage for the session-state tamper guard after it was repointed from the
/tmp control file to <sessions>/<id>/state.json.

The guard blocks a hook-external Write or shell command that targets a session
state.json directly, so a subagent can't rewrite control state behind the
store's back — while the session hooks themselves pass through the
CLAUDE_SESSION_HOOK bypass. Each case calls main() in this process and asserts
the exit code (2 = blocked, 0 = allowed); the first block and the first allow
stay a real `python3 <hook>` run, because the harness reads the exit code off
the process rather than off a return value.
"""

import io
import json
import os
import subprocess
import sys

import protect_session_state
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "protect_session_state.py")
STATE_PATH = os.path.expanduser("~/.claude/sessions/abc123/state.json")
SUBAGENT_STATE = os.path.expanduser("~/.claude/sessions/parent/subagents/agent-x/state.json")


def _run(monkeypatch, payload, hook_bypass=False, process=False):
    if process:
        env = dict(os.environ)
        env.pop("CLAUDE_SESSION_HOOK", None)
        if hook_bypass:
            env["CLAUDE_SESSION_HOOK"] = "true"
        return subprocess.run(
            ["python3", HOOK], input=json.dumps(payload), text=True,
            capture_output=True, env=env,
        ).returncode
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    if hook_bypass:
        monkeypatch.setenv("CLAUDE_SESSION_HOOK", "true")
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return protect_session_state.main()


def test_blocks_direct_write_to_state_json(monkeypatch):
    assert _run(monkeypatch, {"tool_input": {"file_path": STATE_PATH}}, process=True) == 2




def test_blocks_shell_write_to_state_json(monkeypatch):
    assert _run(monkeypatch, {"tool_input": {"command": "echo {} > %s" % STATE_PATH}}) == 2




def test_unrelated_file_path_allowed(monkeypatch):
    assert _run(monkeypatch, {"tool_input": {"file_path": "packages/agents/hooks/foo.py"}},
                process=True) == 0


def test_unrelated_command_allowed(monkeypatch):
    assert _run(monkeypatch, {"tool_input": {"command": "echo hello world"}}) == 0
