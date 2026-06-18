"""Coverage for the proposing-state interpreter-execution block.

While a proposal is expected, block_edits_during_proposal blocks executing a bare
interpreter — inline or on a script file — but leaves the named tools (codex,
codex-run, trace, git, uv/pytest, npm) running, and never blocks *writing* a
script: it allows the write and emits a heads-up that execution is blocked.

Each case runs the hook as a subprocess against an isolated proposing-state spine
and asserts the exit code (2 = blocked, 0 = allowed).
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "block_edits_during_proposal.py")
SID = "test_interp_block"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


@pytest.fixture
def proposing(tmp_path, monkeypatch):
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    d = tmp_path / "sessions" / SID
    d.mkdir(parents=True)
    (d / "state.json").write_text(json.dumps({"state": "proposing"}))


def _run(tool_input):
    payload = json.dumps({"session_id": SID, "cwd": REPO, "tool_input": tool_input})
    return subprocess.run(["python3", HOOK], input=payload, text=True, capture_output=True, cwd=REPO)


# Interpreter executions — inline and script-file — must block (exit 2)
INTERP_BLOCKED = [
    'python3 -c "x=1"',
    "python3 /tmp/fix.py",
    'node -e "x"',
    "node /tmp/x.js",
    'perl -e "x"',
    "ruby /tmp/x.rb",
    "bash /tmp/x.sh",
    'bash -c "echo hi"',
]

# Named tools — including the codex flow via codex-run — must run (exit 0)
TOOLS_ALLOWED = [
    'codex-run @architect "review this"',
    "codex exec -s read-only 'x'",
    "trace grep foo",
    "git log",
    "uv run pytest tests/hooks/",
    "npm run build",
]


@pytest.mark.parametrize("cmd", INTERP_BLOCKED)
def test_interpreter_execution_blocked(proposing, cmd):
    assert _run({"command": cmd}).returncode == 2


@pytest.mark.parametrize("cmd", TOOLS_ALLOWED)
def test_named_tools_allowed(proposing, cmd):
    assert _run({"command": cmd}).returncode == 0


def test_writing_a_script_is_allowed_with_warning(proposing):
    r = _run({"file_path": "/tmp/fix.py"})
    assert r.returncode == 0
    assert "additionalContext" in r.stdout  # the heads-up is emitted, the write is allowed


def test_writing_a_repo_file_still_blocks(proposing):
    assert _run({"file_path": os.path.join(REPO, "scratch.py")}).returncode == 2
