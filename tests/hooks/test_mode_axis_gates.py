"""Which agent each mode gates, and what the one-time session migration produces.

Mode gates a DISPATCHED agent on both surfaces. Outside a dispatch it gates the spawn
surface alone, so the architect's own writes stay on the state axis — proposing holds
them back, executing lets him do whatever — while a build session there is refused a
spawn like any other. That split is what these cases pin: the same seeded orchestrate
session refuses a dispatched orchestrator's edit and permits the architect's.

For a dispatched orchestrator the forbidden surface is MUTATIONS, not Bash: it has to
be able to run the suite and read the tree to judge what its subagents built.

The retired spellings (state proposing/executing, approach solo/subagents) are not
read anywhere any more. They are covered here as MIGRATION cases: an old file goes in,
the new shape comes out, and the capabilities the old file described still hold.

The gates resolve every input — the session, the mode declaration, the dispatch
marker — from the payload and from os.environ at call time, so a seeded environment
and a direct main() call decide exactly what a spawned process decides. Four cases
still spawn, one blocking and one allowing per gate, because the harness reads an
exit code off a process.
"""

import io
import json
import os
import subprocess
import sys

import pytest
from conftest import PY_HOOKS

sys.path.insert(0, PY_HOOKS)
import block_spawning  # noqa: E402
import block_writes  # noqa: E402
from lib.session_state import migrate_sessions  # noqa: E402

WRITES = block_writes
SPAWNING = block_spawning
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

SID = "mode-axis-session"
ALLOW, BLOCK = 0, 2


@pytest.fixture
def session(tmp_path, monkeypatch):
    """Seed one main session's state.json and hand back a runner for the gates."""
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", SID)
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    session_dir = tmp_path / "sessions" / SID
    session_dir.mkdir(parents=True)
    state_file = session_dir / "state.json"

    def seed(**fields):
        state_file.write_text(json.dumps(dict({"session_id": SID, "role": "main"}, **fields)))

    def agent_file(mode):
        """One agent definition, named for the mode it declares (or none at all)."""
        name = "dispatched-%s" % (mode or "undeclared")
        path = tmp_path / "config" / "agents" / (name + ".md")
        path.parent.mkdir(parents=True, exist_ok=True)
        body = "---\nname: %s\n" % name + (("mode: %s\n" % mode) if mode else "")
        path.write_text(body + "---\n\nFrame.\n")
        return name, str(path)

    def run(hook, tool_name, tool_input, dispatched_as=..., sidechain_as=...,
            teammate=False, teammate_as=..., missing_agent="", spawn=False):
        """Run one gate against one shape of caller.

        `dispatched_as` names the mode a codex run declares, `sidechain_as` the mode
        a Claude subagent declares, `missing_agent` names a subagent whose name has
        no roster file behind it, and `teammate` marks a hand-managed top-level
        agent — an agentId with no sidechain marker. `teammate_as` is that same
        teammate with an agent named on the payload, the way the harness names one a
        session was started on, declaring the given mode. Pass none of them for the
        architect's own plain session. `spawn` runs the gate as its own process.

        Every environment variable is set or deleted on each call, never left from
        the last one, because monkeypatch holds its writes for the whole test.
        """
        payload = {"session_id": SID, "cwd": REPO,
                   "tool_name": tool_name, "tool_input": tool_input}
        if teammate or teammate_as is not ...:
            payload["agentId"] = "agent-abc"
        if teammate_as is not ...:
            payload["agent_type"] = agent_file(teammate_as)[0]
        if dispatched_as is not ...:
            monkeypatch.setenv("CODEX_RUN_AGENT_FILE", agent_file(dispatched_as)[1])
        else:
            monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
        if sidechain_as is not ...:
            payload["isSidechain"] = True
            payload["agentId"] = "agent-abc"
            payload["agent_type"] = agent_file(sidechain_as)[0]
        if missing_agent:
            payload["isSidechain"] = True
            payload["agent_type"] = missing_agent
        if sidechain_as is not ... or missing_agent or teammate_as is not ...:
            monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "config"))
        else:
            monkeypatch.delenv("CLAUDE_CONFIG_DIR", raising=False)
        if spawn:
            return subprocess.run(
                ["python3", os.path.join(PY_HOOKS, hook.__name__ + ".py")],
                input=json.dumps(payload), text=True, capture_output=True, cwd=REPO,
            ).returncode
        monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
        return hook.main()

    seed.run = run
    seed.state_file = state_file
    return seed


# ---------------------------------------------------------------------------
# a dispatched orchestrator: spawns, validates, mutates nothing
# ---------------------------------------------------------------------------

def test_dispatched_orchestrator_spawns(session):
    """Spawned: the allowing exit code the harness reads off block_spawning."""
    session(state="execute", mode="build", mode_typed=True)
    assert session.run(SPAWNING, "Bash", {"command": 'codex-run @cto "y"'},
                       dispatched_as="orchestrate", spawn=True) == ALLOW


def test_dispatched_orchestrator_own_edit_is_blocked(session):
    """Spawned: the blocking exit code the harness reads off block_writes."""
    session(state="execute", mode="build", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       dispatched_as="orchestrate", spawn=True) == BLOCK






# ---------------------------------------------------------------------------
# a dispatched executor: writes, never spawns
# ---------------------------------------------------------------------------

def test_dispatched_executor_writes(session):
    session(state="propose", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Write", {"file_path": os.path.join(REPO, "note.txt")},
                       dispatched_as="build") == ALLOW


def test_dispatched_executor_does_not_spawn(session):
    session(state="execute", mode="orchestrate", mode_typed=True)
    # Spawned: the blocking exit code the harness reads off block_spawning.
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"},
                       dispatched_as="build", spawn=True) == BLOCK
    assert session.run(SPAWNING, "Bash", {"command": 'codex-run @cto "y"'},
                       dispatched_as="build") == BLOCK




# ---------------------------------------------------------------------------
# a sidechain agent: the same policy the codex path answers from
# ---------------------------------------------------------------------------





# ---------------------------------------------------------------------------
# a name with no roster file behind it: the omission fallback governs, so the
# agent runs as an executor. It writes, it never spawns, and nothing bricks.
# ---------------------------------------------------------------------------



# ---------------------------------------------------------------------------
# a hand-managed teammate: an agentId with no sidechain marker is one of the
# architect's own top-level agents — stage-governed when it writes, mode-gated when
# it spawns
# ---------------------------------------------------------------------------











# ---------------------------------------------------------------------------
# the architect's own session: the state axis governs its writes, the mode its spawns
# ---------------------------------------------------------------------------

def test_main_session_executing_writes_under_any_mode(session):
    session(state="execute", mode="orchestrate", mode_typed=True)
    # Spawned: the allowing exit code the harness reads off block_writes.
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       spawn=True) == ALLOW
    assert session.run(WRITES, "Bash", {"command": "echo hi > note.txt"}) == ALLOW


def test_main_session_proposing_still_blocks_its_edits(session):
    session(state="propose", mode="build", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")}) == BLOCK




@pytest.mark.parametrize("mode,expected", [("orchestrate", ALLOW), ("build", BLOCK)])
def test_main_session_spawns_under_the_mode_it_records(session, mode, expected):
    """The mode the architect typed is his instruction, and this gate is what makes
    it one — the session he typed /build into does not spawn either."""
    session(state="execute", mode=mode, mode_typed=True)
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"}) == expected


# ---------------------------------------------------------------------------
# the roster: every agent declares its own mode, so the omission fallback is
# never what decides a real dispatch
# ---------------------------------------------------------------------------



# ---------------------------------------------------------------------------
# the migration: old file in, new file out, capabilities intact
# ---------------------------------------------------------------------------

def _migrated(session, **legacy):
    session(**legacy)
    assert migrate_sessions() == 1
    return json.loads(session.state_file.read_text())
















