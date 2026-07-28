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


# --- a dispatched codex agent is not the architect's conversation -------------

def _run_as(tool_input, agent_file, tool_name="Bash"):
    env = dict(os.environ)
    if agent_file:
        env["CODEX_RUN_AGENT_FILE"] = agent_file
    else:
        env.pop("CODEX_RUN_AGENT_FILE", None)
    payload = json.dumps({"session_id": SID, "cwd": REPO,
                          "tool_name": tool_name, "tool_input": tool_input})
    return subprocess.run(["python3", HOOK], input=payload, text=True,
                          capture_output=True, env=env).returncode


AGENT = os.path.join(REPO, "packages", "agents", "agents", "ponytail.md")


@pytest.mark.parametrize("command", [
    "python3 scripts/sync.py",
    "node build.js",
    "echo hi > packages/out.txt",
])
def test_a_dispatched_codex_agent_is_not_gated(proposing, command):
    """Its task arrived already scoped, so there is no proposal it is holding up.

    The state it carries is the intent classifier's read of its dispatch prompt,
    which defaults to proposing — gating on that refused a codex agent every
    interpreter and every in-repo write for its whole run.
    """
    assert _run_as({"command": command}, AGENT) == 0


@pytest.mark.parametrize("command", [
    "python3 scripts/sync.py",
    "echo hi > packages/out.txt",
])
def test_a_session_that_is_not_a_dispatched_agent_is_still_gated(proposing, command):
    assert _run_as({"command": command}, None) == 2


_PATCH = ("*** Begin Patch\n*** Update File: docs/x.md\n@@\n"
          "+echo hi > packages/out.txt\n*** End Patch")


def test_a_patch_body_is_not_parsed_as_shell(proposing):
    """codex delivers apply_patch as tool_input.command carrying the patch body.

    Parsed as shell, a `+` line adding a redirect reads as a redirect, so a
    legitimate edit was refused for text inside its own diff — and a patch that
    rewrote repo files passed, because it carried no shell syntax. The write is
    what this gate cares about, and it is blocked as a write.
    """
    assert _run_as({"command": _PATCH}, None, tool_name="apply_patch") == 2
    assert _run_as({"command": _PATCH}, AGENT, tool_name="apply_patch") == 0
