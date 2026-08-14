"""Coverage for record_session_event.py — the one hook that reads a Claude Code
event and records the matching session-state change through the spine.

It centralizes the session-state recording that settings.json wires for each
Claude Code event. The tests feed real event payloads and assert the resulting
on-disk state.json is exactly what the spine produces today, plus the
completed-task-notification archive path, and the never-block contract.

The store is isolated under tmp_path via CLAUDE_DATA_ROOT (same as the spine
tests). The hook imports the spine as `lib.session_state` and the archive as
`archive_subagent_log`; we drive the hook through its `main()` by feeding the
payload on stdin, and monkeypatch the spine's clock for determinism.
"""

import io
import json
import os
import sys

import pytest
from conftest import PY_HOOKS

# The hook imports `from lib.session_state import ...`; the spine module also
# refers to itself as a bare `session_state`. Put the lib dir on the path and pin
# the same module object under both names so a clock patch on one is seen by both.
sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

import record_session_event  # noqa: E402
from lib import session_state  # noqa: E402


@pytest.fixture
def root(tmp_path, monkeypatch):
    data_root = tmp_path / "claude"
    projects_root = tmp_path / "projects"
    projects_root.mkdir(parents=True)
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(data_root))
    monkeypatch.setenv("CLAUDE_PROJECTS_ROOT", str(projects_root))
    for var in ("AGENT_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID",
                "CODEX_RUN_AGENT_FILE", "CLAUDE_CONFIG_DIR"):
        monkeypatch.delenv(var, raising=False)
    return data_root


@pytest.fixture
def clock(monkeypatch):
    state = {"now": 1000}
    monkeypatch.setattr(session_state, "_now", lambda: state["now"])
    return lambda v: state.update(now=v)


def _fire(payload, monkeypatch):
    """Feed one event payload to the hook on stdin; return its exit code."""
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps(payload)))
    return record_session_event.main()


def _state(root, *parts):
    return os.path.join(str(root), "sessions", *parts, "state.json")


def _read(path):
    with open(path) as fh:
        return json.load(fh)


# ---------------------------------------------------------------------------
# The recording commands: event in -> spine result out
# ---------------------------------------------------------------------------







# ---------------------------------------------------------------------------
# The mode axis: SessionStart records the event agent's declared mode, unless
# the architect already typed one in that session
# ---------------------------------------------------------------------------

def _declare_agent(tmp_path, monkeypatch, mode):
    """One agent definition under a config root the hook will resolve names in.
    Returns the name to put on the payload as `agent_type`."""
    name = "declares-%s" % (mode or "nothing")
    path = tmp_path / "config" / "agents" / (name + ".md")
    path.parent.mkdir(parents=True, exist_ok=True)
    body = "---\nname: %s\n" % name + (("mode: %s\n" % mode) if mode else "")
    path.write_text(body + "---\n\nFrame.\n")
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "config"))
    return name


def test_session_start_records_the_declared_mode(root, tmp_path, clock, monkeypatch):
    agent = _declare_agent(tmp_path, monkeypatch, "orchestrate")
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "m1",
                "transcript_path": "/p/m1.jsonl", "agent_type": agent}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "m1"))
    assert st["mode"] == "orchestrate"
    assert st["mode_typed"] is False


def test_session_start_records_the_fallback_mode_when_nothing_is_declared(root, clock, monkeypatch):
    """No agent names the session, so the omission fallback is what lands — over an
    untyped orchestrate the hook itself recorded earlier."""
    _fire({"hook_event_name": "SessionStart", "session_id": "m2",
           "transcript_path": "/p/m2.jsonl"}, monkeypatch)
    session_state.merge_state("m2", {"mode": "orchestrate", "mode_typed": False})
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "m2",
                "transcript_path": "/p/m2.jsonl"}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "m2"))
    assert st["mode"] == "build"
    assert st["mode_typed"] is False


def test_session_start_keeps_a_typed_mode(root, tmp_path, clock, monkeypatch):
    """The architect typed a mode in this session; a later SessionStart — a resume
    or a compact — must not put the declaration back over it."""
    _fire({"hook_event_name": "SessionStart", "session_id": "m3",
           "transcript_path": "/p/m3.jsonl"}, monkeypatch)
    session_state.merge_state("m3", {"mode": "interview", "mode_typed": True})
    agent = _declare_agent(tmp_path, monkeypatch, "orchestrate")
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "m3",
                "transcript_path": "/p/m3.jsonl", "agent_type": agent}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "m3"))
    assert st["mode"] == "interview"
    assert st["mode_typed"] is True


def test_session_start_rewrites_a_mode_nobody_typed(root, tmp_path, clock, monkeypatch):
    """An untyped mode is the hook's own record, so a restart under a different
    agent replaces it."""
    _fire({"hook_event_name": "SessionStart", "session_id": "m4",
           "transcript_path": "/p/m4.jsonl"}, monkeypatch)
    assert _read(_state(root, "m4"))["mode"] == "build"
    agent = _declare_agent(tmp_path, monkeypatch, "interview")
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "m4",
                "transcript_path": "/p/m4.jsonl", "agent_type": agent}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "m4"))
    assert st["mode"] == "interview"
    assert st["mode_typed"] is False


# ---------------------------------------------------------------------------
# task-notification: completed -> tracer-log archive; else nothing
# ---------------------------------------------------------------------------

def _start_parent_and_subagent(root, monkeypatch, parent, agent_task):
    """Establish a parent main session and a subagent under it via the spine."""
    _fire({"hook_event_name": "SessionStart", "session_id": parent,
           "transcript_path": "/p/%s/%s.jsonl" % (parent, parent)}, monkeypatch)
    session_state.cmd_start(["agent-%s" % agent_task, "--transcript-path",
                             "/p/%s/subagents/agent-%s.jsonl" % (parent, agent_task)])


def _notification(task_id, status):
    return ("<task-notification><task-id>%s</task-id>"
            "<status>%s</status></task-notification>") % (task_id, status)


def _seed_tracer_log(monkeypatch, session_id, task_id):
    """Create a fake repo with a live tracer-log dir for the subagent, so the
    archive hook has something to move. Returns (active_dir, archived_dir)."""
    import subprocess
    import tempfile
    repo = tempfile.mkdtemp()
    subprocess.run(["git", "-C", repo, "init", "-q"], check=True)
    monkeypatch.chdir(repo)
    base = os.path.join(repo, ".tracer-cache", "sessions", session_id)
    active = os.path.join(base, task_id)
    os.makedirs(active)
    with open(os.path.join(active, "events.jsonl"), "w") as fh:
        fh.write("{}\n")
    return active, os.path.join(base, "archived", task_id)








# ---------------------------------------------------------------------------
# Never-block contract: exit 0 on every degenerate path
# ---------------------------------------------------------------------------





