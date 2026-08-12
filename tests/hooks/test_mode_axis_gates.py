"""Which agent each mode gates, and what the one-time session migration produces.

Mode gates a DISPATCHED agent on both surfaces. The architect's own session runs on
the state axis — proposing holds it back, executing lets it do whatever — so the mode
a main session records drives the statusline and which skill loads, never a refusal.
That split is what these cases pin: the same seeded orchestrate session refuses a
dispatched orchestrator's edit and permits the architect's.

His hand-managed teammates sit across the split: their writes are the stage's call
like his own, and their spawns answer to their agent's declaration like a dispatch,
because one spawning agent cascades usage through everything it starts.

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
from lib import agent_memory  # noqa: E402
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


@pytest.mark.parametrize("command", [
    "echo hi", "git status", "ls -la", "cat README.md",
    "uv run pytest tests/hooks", "pytest tests/hooks", "npm test",
])
def test_dispatched_orchestrator_reads_and_validates(session, command):
    """The write surface is mutations, not Bash: an orchestrator that cannot run the
    suite cannot judge what its subagents built."""
    session(state="execute", mode="build", mode_typed=True)
    assert session.run(WRITES, "Bash", {"command": command},
                       dispatched_as="orchestrate") == ALLOW


@pytest.mark.parametrize("command", ["echo hi > note.txt", "rm note.txt",
                                     "cp a.txt note.txt", "git rm note.txt",
                                     "ssh prod 'rm note.txt'",
                                     "bash -c 'rm note.txt'"])
def test_dispatched_orchestrator_mutations_are_blocked(session, command):
    session(state="execute", mode="build", mode_typed=True)
    assert session.run(WRITES, "Bash", {"command": command},
                       dispatched_as="orchestrate") == BLOCK


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


def test_claude_subagent_writes_under_an_orchestrate_session(session):
    """A Claude subagent is gated by its own declaration, never its parent's mode."""
    session(state="propose", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       sidechain_as="build") == ALLOW


# ---------------------------------------------------------------------------
# a sidechain agent: the same policy the codex path answers from
# ---------------------------------------------------------------------------

def test_sidechain_orchestrator_spawns_validates_and_mutates_nothing(session):
    session(state="execute", mode="build", mode_typed=True)
    assert session.run(SPAWNING, "Bash", {"command": 'codex-run @x "y"'},
                       sidechain_as="orchestrate") == ALLOW
    assert session.run(WRITES, "Bash", {"command": "uv run pytest tests/hooks"},
                       sidechain_as="orchestrate") == ALLOW
    assert session.run(WRITES, "Bash", {"command": "git status"},
                       sidechain_as="orchestrate") == ALLOW
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       sidechain_as="orchestrate") == BLOCK
    assert session.run(WRITES, "Bash", {"command": "echo hi > note.txt"},
                       sidechain_as="orchestrate") == BLOCK


def test_sidechain_builder_writes_and_never_spawns(session):
    session(state="execute", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       sidechain_as="build") == ALLOW
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"},
                       sidechain_as="build") == BLOCK
    assert session.run(SPAWNING, "Bash", {"command": 'codex-run @x "y"'},
                       sidechain_as="build") == BLOCK


# ---------------------------------------------------------------------------
# a name with no roster file behind it: the omission fallback governs, so the
# agent runs as an executor. It writes, it never spawns, and nothing bricks.
# ---------------------------------------------------------------------------

def test_sidechain_with_no_roster_file_runs_as_an_executor(session):
    session(state="execute", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       missing_agent="no-such-agent") == ALLOW
    assert session.run(WRITES, "Bash", {"command": "uv run pytest tests/hooks"},
                       missing_agent="no-such-agent") == ALLOW
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"},
                       missing_agent="no-such-agent") == BLOCK


# ---------------------------------------------------------------------------
# a hand-managed teammate: an agentId with no sidechain marker is one of the
# architect's own top-level agents — stage-governed when it writes, mode-gated when
# it spawns
# ---------------------------------------------------------------------------

def test_teammate_is_held_to_the_stage_axis(session):
    """A teammate is main-like, so the stage governs its edits exactly as it governs
    the architect's own session: proposing refuses them, executing lets them through."""
    session(state="propose", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       teammate=True) == BLOCK
    assert session.run(WRITES, "Bash", {"command": "echo hi > note.txt"},
                       teammate=True) == BLOCK
    session(state="execute", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       teammate=True) == ALLOW
    assert session.run(WRITES, "Bash", {"command": "echo hi > note.txt"},
                       teammate=True) == ALLOW


@pytest.mark.parametrize("dispatch", [{"sidechain_as": "build"}, {"dispatched_as": "build"}])
def test_a_dispatched_agent_is_exempt_from_the_proposing_stage(session, dispatch):
    """Its task arrived already scoped, so the launcher's pending proposal is not its
    to hold up — on the Claude sidechain and on the codex path alike."""
    session(state="propose", mode="orchestrate", mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       **dispatch) == ALLOW


@pytest.mark.parametrize("mode", ["orchestrate", "build", "interview"])
def test_teammate_writes_are_never_mode_gated(session, mode):
    """The write surface reads the stage and nothing else, whatever the teammate's
    own agent declares and whatever mode the session records."""
    session(state="execute", mode=mode, mode_typed=True)
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       teammate=True) == ALLOW
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")},
                       teammate_as=mode) == ALLOW


@pytest.mark.parametrize("mode,verdict", [("orchestrate", ALLOW), ("build", BLOCK),
                                          ("interview", BLOCK)])
def test_teammate_spawns_only_when_its_agent_orchestrates(session, mode, verdict):
    """Spawning cascades usage through every agent it starts, so a teammate answers
    for it from its own declaration exactly as a dispatched agent does."""
    session(state="execute", mode="orchestrate", mode_typed=True)
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"},
                       teammate_as=mode) == verdict
    assert session.run(SPAWNING, "Bash", {"command": 'codex-run @x "y"'},
                       teammate_as=mode) == verdict


def test_teammate_with_no_roster_file_cannot_spawn(session):
    """The omission fallback is build, which refuses the spawn."""
    session(state="execute", mode="orchestrate", mode_typed=True)
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"},
                       teammate=True) == BLOCK


# ---------------------------------------------------------------------------
# the architect's own session: the state axis governs, mode never refuses
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


def test_main_session_proposing_keeps_the_docs_escape_hatch(session):
    session(state="propose", mode="build", mode_typed=True)
    assert session.run(
        WRITES, "Write",
        {"file_path": os.path.join(REPO, "docs", "agents", "run", "report.md")}) == ALLOW


@pytest.mark.parametrize("mode", ["orchestrate", "build", "interview"])
def test_main_session_spawns_under_every_mode(session, mode):
    """Spawning is the architect's call, never a mode's."""
    session(state="execute", mode=mode, mode_typed=True)
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"}) == ALLOW


# ---------------------------------------------------------------------------
# the roster: every agent declares its own mode, so the omission fallback is
# never what decides a real dispatch
# ---------------------------------------------------------------------------

def test_every_roster_agent_declares_a_mode():
    roster = os.path.join(REPO, "packages", "agents", "agents")
    undeclared = []
    for name in sorted(os.listdir(roster)):
        if not name.endswith(".md") or name.endswith(".prompt.md"):
            continue
        declared = agent_memory.declaration(os.path.join(roster, name), "mode")
        if declared not in ("orchestrate", "build", "interview"):
            undeclared.append(name)
    assert undeclared == []


# ---------------------------------------------------------------------------
# the migration: old file in, new file out, capabilities intact
# ---------------------------------------------------------------------------

def _migrated(session, **legacy):
    session(**legacy)
    assert migrate_sessions() == 1
    return json.loads(session.state_file.read_text())


def test_legacy_executing_subagents_session_migrates(session):
    """The architect's orchestrating sessions come out orchestrating, and keep writing."""
    state = _migrated(session, state="executing", approach="subagents")
    assert state["state"] == "execute"
    assert state["mode"] == "orchestrate"
    assert "approach" not in state
    assert session.run(SPAWNING, "Agent", {"subagent_type": "cto"}) == ALLOW
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")}) == ALLOW


def test_legacy_solo_session_migrates_to_build(session):
    state = _migrated(session, state="executing", approach="solo")
    assert state["state"] == "execute"
    assert state["mode"] == "build"
    assert "approach" not in state
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")}) == ALLOW


def test_legacy_proposing_session_migrates_and_still_blocks_edits(session):
    state = _migrated(session, state="proposing", approach="solo")
    assert state["state"] == "propose"
    assert session.run(WRITES, "Edit", {"file_path": os.path.join(REPO, "note.txt")}) == BLOCK


def test_legacy_interview_state_migrates_onto_the_mode_axis(session):
    state = _migrated(session, state="interview", approach="solo")
    assert state["state"] == "propose"
    assert state["mode"] == "interview"


def test_unknown_stage_heals_to_the_default(session):
    state = _migrated(session, state="auto", mode="orchestrate", mode_typed=True)
    assert state["state"] == "propose"
    assert state["mode"] == "orchestrate"


def test_migration_is_idempotent(session):
    session(state="executing", approach="subagents")
    assert migrate_sessions() == 1
    assert migrate_sessions() == 0


def test_migration_reaches_subagent_nested_state_files(session, tmp_path):
    session(state="propose", mode="build", mode_typed=True)
    nested = tmp_path / "sessions" / SID / "subagents" / "agent-nested"
    nested.mkdir(parents=True)
    (nested / "state.json").write_text(
        json.dumps({"session_id": "agent-nested", "role": "subagent",
                    "state": "executing", "approach": "subagents"})
    )
    assert migrate_sessions() == 1
    state = json.loads((nested / "state.json").read_text())
    assert state["state"] == "execute"
    assert state["mode"] == "orchestrate"
    assert "approach" not in state


def test_migration_leaves_an_unparseable_file_for_the_healer(session):
    """Never delete: a corrupt file is skipped here and healed by _ensure_session."""
    session.state_file.write_text("{not json")
    assert migrate_sessions() == 0
    assert session.state_file.read_text() == "{not json"
