"""Coverage for the spawn guard.

block_spawning answers from lib.session_mode, the same policy the write gate reads,
so the two gates cannot drift apart. This file pins both halves of that answer:

- A DISPATCHED executor may not start another agent, whether the route is a spawn
  tool or a shell command naming one of the three spawn commands, and a line whose
  head cannot be read is blocked because hiding the head was the bypass.
- The architect's own session is never spawn-gated, whatever mode it records.
  Spawning is his call to make. His hand-managed teammates — a payload carrying an
  agentId with no sidechain marker — are gated here instead by what their own agent
  declares, because one spawning agent cascades usage through everything it starts.
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
import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "block_spawning.py")
SID = "test_block_spawning"

ALLOW, BLOCK = 0, 2


def _run(payload, tmp_path, monkeypatch, mode="build", dispatched=True,
         teammate=False, spawn=False):
    """Run the guard. `dispatched` marks a codex run declaring `mode` — pass None as
    the mode for a run whose definition declares none. Without `dispatched` the
    payload is the architect's own session, which records `mode` and is never gated.
    `teammate` names the mode declared by the agent one of his hand-managed top-level
    agents runs, adding the agentId such a payload carries and the agent name the
    harness puts on it. `spawn` runs the guard as its own process."""
    root = tmp_path / "claude"
    session = root / "sessions" / SID
    session.mkdir(parents=True, exist_ok=True)
    (session / "state.json").write_text(
        json.dumps({"session_id": SID, "role": "main", "state": "execute",
                    "mode": mode or "build", "mode_typed": True})
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


def test_executor_is_blocked_from_the_codex_spawn_tool(tmp_path, monkeypatch):
    """codex's own spawn tool, since the guard runs on both harnesses."""
    assert _run({"tool_name": "spawn_agent", "tool_input": {}}, tmp_path,
                monkeypatch) == BLOCK


@pytest.mark.parametrize("command", [
    'codex-run @architect "review this"',
    "codex exec -s read-only 'x'",
    "claude -p 'do x'",
])
def test_executor_is_blocked_from_the_spawn_commands(tmp_path, monkeypatch, command):
    """One case per member of the guard's own `_SPAWNS` set."""
    assert _run(_bash(command), tmp_path, monkeypatch) == BLOCK


def test_executor_is_blocked_behind_a_prefix_word(tmp_path, monkeypatch):
    """A prefix word whose flag arity is unknown leaves the command a candidate,
    and a candidate in `_SPAWNS` still blocks."""
    assert _run(_bash("xargs -n1 codex-run"), tmp_path, monkeypatch) == BLOCK


def test_executor_is_blocked_on_a_command_it_cannot_parse(tmp_path, monkeypatch):
    """An unbalanced quote hides the head, and hiding the head was the bypass."""
    assert _run(_bash("codex-run @architect 'unbalanced"), tmp_path, monkeypatch) == BLOCK


def test_executor_still_runs_ordinary_bash(tmp_path, monkeypatch):
    """Spawned: the allowing exit code the harness reads off this guard."""
    assert _run(_bash("git log"), tmp_path, monkeypatch, spawn=True) == ALLOW
    assert _run(_bash("trace grep foo"), tmp_path, monkeypatch) == ALLOW
    # A spawn command named in an argument is not a command head.
    assert _run(_bash("echo codex-run"), tmp_path, monkeypatch) == ALLOW


def test_executor_is_not_blocked_by_claude_inside_a_path(tmp_path, monkeypatch):
    # 'claude' inside ~/.claude is not a command head — must not false-block.
    assert _run(_bash("cat ~/.claude/sessions/x/state.json"), tmp_path,
                monkeypatch) == ALLOW


# ---------------------------------------------------------------------------
# a dispatched orchestrator: spawning is its whole job
# ---------------------------------------------------------------------------

def test_orchestrator_spawns(tmp_path, monkeypatch):
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                mode="orchestrate") == ALLOW
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch,
                mode="orchestrate") == ALLOW


# ---------------------------------------------------------------------------
# the architect's own session: never spawn-gated
# ---------------------------------------------------------------------------

def test_main_session_spawns(tmp_path, monkeypatch):
    """`is_dispatched` is false here, so the guard returns before it reads a mode —
    which is why one mode covers all three. test_mode_axis_gates pins that the
    recorded mode never changes the answer."""
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                dispatched=False) == ALLOW
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch,
                dispatched=False) == ALLOW


def test_hand_managed_teammate_answers_for_its_own_agent(tmp_path, monkeypatch):
    """An agentId with no sidechain marker is the architect's own top-level agent, and
    on the spawn surface it is gated by what that agent declares — an orchestrator
    starts others, a builder does not. Its writes stay on the stage axis, which
    test_mode_axis_gates pins."""
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                dispatched=False, teammate="orchestrate") == ALLOW
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch,
                dispatched=False, teammate="orchestrate") == ALLOW
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                dispatched=False, teammate="build") == BLOCK
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch,
                dispatched=False, teammate="build") == BLOCK


# ---------------------------------------------------------------------------
# an agent whose definition declares no mode: the omission fallback is build, so
# it is gated as an executor rather than running ungated
# ---------------------------------------------------------------------------

def test_undeclared_agent_is_gated_as_an_executor(tmp_path, monkeypatch):
    assert _run({"tool_name": "Agent", "tool_input": {}}, tmp_path, monkeypatch,
                mode=None) == BLOCK
    assert _run(_bash('codex-run @x "y"'), tmp_path, monkeypatch, mode=None) == BLOCK
