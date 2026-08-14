"""Coverage for the spawn guard.

block_spawning answers from lib.session_mode, the same policy the write gate reads,
so the two gates cannot drift apart. This file pins both halves of that answer:

- A DISPATCHED executor may not start another agent, whether the route is a spawn
  tool or a shell command naming one of the three spawn commands, and a line whose
  head cannot be read is blocked because hiding the head was the bypass.
- A session outside a dispatch is gated by the mode governing it: the agent it was
  started on, or the mode the architect typed into it. Build refuses the spawn there
  exactly as it does inside a dispatch, because the mode is the instruction and this
  gate is what makes it one.
- An agent whose definition declares no mode falls back to build and is gated as an
  executor; the roster carries an explicit declaration per agent so that fallback is
  never what governs a real dispatch.

The shell shapes themselves — a leading space, a second line, `env FOO=1`, an
absolute path, `bash -c '…'`, `sudo`, `timeout`, a command named in an argument —
are lib.command's contract and are asserted string by string in
test_command_parsing. The hook's own contribution is one membership test against
`_SPAWNS`, so only the cases that reach a decision of its own live here.

Each case calls the guard's main() against an isolated CLAUDE_DATA_ROOT; the guard
resolves its session and its dispatch marker from os.environ at call time. Two
cases spawn the guard, one blocking and one allowing, because the harness reads an
exit code off a process.
"""

import io
import json
import os
import subprocess
import sys

import block_spawning
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "block_spawning.py")
SID = "test_block_spawning"

ALLOW, BLOCK = 0, 2


def _run(payload, tmp_path, monkeypatch, mode="build", dispatched=True,
         teammate=False, spawn=False, typed=True):
    """Run the guard. `dispatched` marks a codex run declaring `mode` — pass None as
    the mode for a run whose definition declares none. Without `dispatched` the
    payload is a top-level session recording `mode` as the one the architect typed.
    `teammate` names the mode declared by the agent such a session was started on,
    adding the agentId and the agent name the harness puts on its payload. `spawn`
    runs the guard as its own process, and `typed` says whether the recorded mode is
    one the architect typed or the one the session's agent declared at startup."""
    root = tmp_path / "claude"
    session = root / "sessions" / SID
    session.mkdir(parents=True, exist_ok=True)
    (session / "state.json").write_text(
        json.dumps({"session_id": SID, "role": "main", "state": "execute",
                    "mode": mode or "build", "mode_typed": typed})
    )
    if teammate:
        definition = tmp_path / "config" / "agents" / "teammate.md"
        definition.parent.mkdir(parents=True, exist_ok=True)
        definition.write_text(
            "---\nname: teammate\nmode: %s\n---\n\nFrame.\n" % teammate)
        monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "config"))
        payload = dict(payload, agentId="agent-abc", agent_type="teammate")
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    # The guard resolves the governing session through owner_session, which prefers
    # the environment — the harness running pytest would otherwise lend these cases
    # its own session and its own mode.
    for var in ("AGENT_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID"):
        monkeypatch.delenv(var, raising=False)
    if dispatched:
        definition = tmp_path / "agents" / "dispatched.md"
        definition.parent.mkdir(parents=True, exist_ok=True)
        declared = ("mode: %s\n" % mode) if mode else ""
        definition.write_text("---\nname: dispatched\n%s---\n\nFrame.\n" % declared)
        monkeypatch.setenv("CODEX_RUN_AGENT_FILE", str(definition))
    else:
        monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    body = json.dumps({"session_id": SID, **payload})
    if spawn:
        return subprocess.run(["python3", HOOK], input=body, text=True,
                              capture_output=True).returncode
    monkeypatch.setattr(sys, "stdin", io.StringIO(body))
    return block_spawning.main()


def _bash(command):
    return {"tool_name": "Bash", "tool_input": {"command": command}}


# ---------------------------------------------------------------------------
# a dispatched executor: every route to another agent is closed
# ---------------------------------------------------------------------------

def test_executor_is_blocked_from_the_agent_tool(tmp_path, monkeypatch):
    """Spawned: the blocking exit code the harness reads off this guard."""
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                spawn=True) == BLOCK










def test_executor_cannot_hide_the_spawn_outside_the_command_line(tmp_path, monkeypatch):
    """Both shapes ran codex-run for real from a build session before lib.command
    started refusing a line whose program it cannot read."""
    assert _run(_bash("sh /tmp/spawnprobe.sh"), tmp_path, monkeypatch) == BLOCK
    assert _run(_bash("python3 -c \"import subprocess; subprocess.run(['codex-run'])\""),
                tmp_path, monkeypatch) == BLOCK


def test_executor_still_runs_ordinary_bash(tmp_path, monkeypatch):
    """Spawned: the allowing exit code the harness reads off this guard."""
    assert _run(_bash("git log"), tmp_path, monkeypatch, spawn=True) == ALLOW
    assert _run(_bash("trace grep foo"), tmp_path, monkeypatch) == ALLOW
    # A spawn command named in an argument is not a command head.
    assert _run(_bash("echo codex-run"), tmp_path, monkeypatch) == ALLOW




# ---------------------------------------------------------------------------
# a dispatched orchestrator: spawning is its whole job
# ---------------------------------------------------------------------------

def test_orchestrator_spawns(tmp_path, monkeypatch):
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                mode="orchestrate") == ALLOW
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch,
                mode="orchestrate") == ALLOW


# ---------------------------------------------------------------------------
# a session outside a dispatch: the mode governing it decides, whether that mode
# came off the agent it runs on or off the command the architect typed
# ---------------------------------------------------------------------------

def test_typed_build_session_does_not_spawn(tmp_path, monkeypatch):
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                dispatched=False) == BLOCK
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch,
                dispatched=False) == BLOCK


def test_typed_orchestrate_session_spawns(tmp_path, monkeypatch):
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                mode="orchestrate", dispatched=False) == ALLOW


def test_a_session_with_no_agent_and_no_typed_mode_spawns(tmp_path, monkeypatch):
    """The recorded mode is the startup default here, not a choice, so there is no
    instruction to enforce and the architect keeps his own dispatches."""
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                dispatched=False, typed=False) == ALLOW


def test_session_started_on_a_build_agent_does_not_spawn(tmp_path, monkeypatch):
    """Nothing was typed here: the agent the session runs on is what gates it, and
    the Workflow tool is one of the routes that starts agents."""
    assert _run({"tool_name": "Workflow", "tool_input": {}}, tmp_path, monkeypatch,
                dispatched=False, typed=False, teammate="build") == BLOCK


# ---------------------------------------------------------------------------
# an agent whose definition declares no mode: the omission fallback is build, so
# it is gated as an executor rather than running ungated
# ---------------------------------------------------------------------------

def test_undeclared_agent_is_gated_as_an_executor(tmp_path, monkeypatch):
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                mode=None) == BLOCK
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch, mode=None) == BLOCK
