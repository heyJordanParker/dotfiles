"""Coverage for the solo-mode guard.

Solo mode blocks every way to start another agent: the Agent tool (Claude
subagents) and the codex / codex-run / claude commands from the shell. Outside
solo, all pass. Each case runs the hook as a subprocess against an isolated
CLAUDE_DATA_ROOT and asserts the exit code (2 = blocked, 0 = allowed).
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "enforce_solo_mode.py")
SID = "test_enforce_solo"


def _run(payload, approach, tmp_path):
    root = tmp_path / "claude"
    sess = root / "sessions" / SID
    sess.mkdir(parents=True, exist_ok=True)
    (sess / "state.json").write_text(json.dumps({"approach": approach}))
    env = dict(os.environ)
    env["CLAUDE_DATA_ROOT"] = str(root)
    env.pop("CLAUDE_SESSION_HOOK", None)
    return subprocess.run(
        ["python3", HOOK], input=json.dumps({"session_id": SID, **payload}),
        text=True, capture_output=True, env=env,
    ).returncode


def _bash(cmd):
    return {"tool_name": "Bash", "tool_input": {"command": cmd}}


def test_solo_blocks_agent(tmp_path):
    assert _run({"tool_name": "Agent", "tool_input": {}}, "solo", tmp_path) == 2


@pytest.mark.parametrize("command", [
    'codex-run @architect "review this"',
    'cd /tmp && codex-run @architect "review this"',
    '$(codex-run @architect "review this")',
])
def test_solo_blocks_codex_run(tmp_path, command):
    assert _run(_bash(command), "solo", tmp_path) == 2


def test_solo_blocks_codex(tmp_path):
    assert _run(_bash("codex exec -s read-only 'x'"), "solo", tmp_path) == 2


def test_solo_blocks_claude(tmp_path):
    assert _run(_bash("claude -p 'do x'"), "solo", tmp_path) == 2


@pytest.mark.parametrize("command", [
    ' codex-run @architect "review this"',
    'echo hi\ncodex-run @architect "review this"',
    'env FOO=1 codex-run @architect "review this"',
    "bash -c 'codex-run @architect \"review this\"'",
    '/Users/jordan/.local/bin/codex-run @architect "review this"',
    './codex-run @architect "review this"',
])
def test_solo_blocks_codex_run_in_shell_shapes(tmp_path, command):
    assert _run(_bash(command), "solo", tmp_path) == 2


def test_solo_allows_ordinary_bash(tmp_path):
    assert _run(_bash("git log"), "solo", tmp_path) == 0
    assert _run(_bash("trace grep foo"), "solo", tmp_path) == 0


@pytest.mark.parametrize("command", [
    "echo codex-run",
    "cat docs/codex-run-notes.md",
    "grep codex README.md",
])
def test_solo_allows_subagent_command_mentions(tmp_path, command):
    assert _run(_bash(command), "solo", tmp_path) == 0


def test_solo_allows_claude_substring_in_path(tmp_path):
    # 'claude' inside ~/.claude is not a command head — must not false-block.
    assert _run(_bash("cat ~/.claude/sessions/x/state.json"), "solo", tmp_path) == 0


def test_subagents_mode_allows_everything(tmp_path):
    assert _run({"tool_name": "Agent", "tool_input": {}}, "subagents", tmp_path) == 0
    assert _run(_bash('codex-run @x "y"'), "subagents", tmp_path) == 0
    assert _run(_bash("claude -p 'x'"), "subagents", tmp_path) == 0
