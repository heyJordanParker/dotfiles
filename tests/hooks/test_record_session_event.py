"""Coverage for record_session_event.py — the one hook that reads a Claude Code
event and records the matching session-state change through the spine.

It centralizes the session-state recording that settings.json wires for each
Claude Code event. The tests feed real event payloads and assert the resulting
on-disk state.json is exactly what the spine produces today, plus the
completed-task-notification archive path, the interview-state write, and the
never-block contract.

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
    for var in ("AGENT_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID"):
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

def test_session_start_records_start(root, clock, monkeypatch):
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "s1",
                "transcript_path": "/p/s1.jsonl"}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "s1"))
    assert st["role"] == "main"
    assert st["session_id"] == "s1"
    # transcript path passed through to the spine's transcript sidecar
    with open(os.path.join(str(root), "sessions", "s1", "transcript")) as fh:
        assert fh.read().strip() == "/p/s1.jsonl"


def test_user_prompt_records_prompt(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s2",
           "transcript_path": "/p/s2.jsonl"}, monkeypatch)
    clock(1100)
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "s2",
                "prompt": "fix the bug in PaymentService"}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "s2"))["current_turn_start"] == 1100


def test_user_prompt_filters_system_injected(root, clock, monkeypatch):
    """The spine drops system-injected prompts; the hook must hand it the raw text
    so that filtering happens — a bracket-enclosed single line is system-injected."""
    _fire({"hook_event_name": "SessionStart", "session_id": "s3",
           "transcript_path": "/p/s3.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "s3",
                "prompt": "[system reminder]"}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "s3"))["current_turn_start"] is None


@pytest.mark.parametrize("key", ["skill", "skill_name", "name"])
def test_interview_skill_switches_state(root, clock, monkeypatch, key):
    """The retired jq tried .skill // .skill_name // .name — match all three."""
    sid = "sk-%s" % key
    _fire({"hook_event_name": "SessionStart", "session_id": sid,
           "transcript_path": "/p/%s.jsonl" % sid}, monkeypatch)
    _fire({"hook_event_name": "PreToolUse", "session_id": sid,
           "tool_name": "Skill", "tool_input": {key: "interview"}}, monkeypatch)
    assert _read(_state(root, sid))["state"] == "interview"


def test_other_skill_leaves_state_alone(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s8",
           "transcript_path": "/p/s8.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "s8",
                "tool_name": "Skill", "tool_input": {"skill": "naming"}}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "s8"))["state"] == "proposing"


def test_start_does_not_apply_agent_rewrite(root, clock, monkeypatch):
    """SessionStart records against the plain session id even when agent_id is
    present, matching the old wiring."""
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "plain1",
                "agent_id": "zzz", "transcript_path": "/p/plain1.jsonl"}, monkeypatch)
    assert rc == 0
    assert os.path.isfile(_state(root, "plain1"))
    assert not os.path.isdir(os.path.join(str(root), "sessions", "plain1", "subagents"))


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


def test_completed_notification_archives_tracer_log(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "pn1", "feed")
    active, archived = _seed_tracer_log(monkeypatch, "pn1", "feed")
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "pn1",
                "prompt": _notification("feed", "completed")}, monkeypatch)
    assert rc == 0
    assert not os.path.isdir(active)
    assert os.path.isdir(archived)


def test_noncompleted_notification_does_nothing(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "pn2", "beef")
    active, archived = _seed_tracer_log(monkeypatch, "pn2", "beef")
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "pn2",
                "prompt": _notification("beef", "failed")}, monkeypatch)
    assert rc == 0
    assert os.path.isdir(active)
    assert not os.path.isdir(archived)


def test_plain_prompt_is_not_a_notification(root, clock, monkeypatch):
    """A normal human prompt records a turn and triggers no archive."""
    _start_parent_and_subagent(root, monkeypatch, "pn3", "cafe")
    active, archived = _seed_tracer_log(monkeypatch, "pn3", "cafe")
    clock(1100)
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "pn3",
                "prompt": "regular human prompt"}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "pn3"))["current_turn_start"] == 1100
    assert os.path.isdir(active)
    assert not os.path.isdir(archived)


# ---------------------------------------------------------------------------
# Never-block contract: exit 0 on every degenerate path
# ---------------------------------------------------------------------------

@pytest.mark.parametrize("payload", [
    {},
    {"hook_event_name": "SessionStart"},                       # no session id
    {"hook_event_name": "UserPromptSubmit", "prompt": "hi"},   # no session id
    {"hook_event_name": "PreToolUse", "tool_name": "Skill",
     "tool_input": {"skill": "interview"}},                    # no session id
    {"hook_event_name": "SomethingUnknown", "session_id": "x"},
    {"hook_event_name": "PreToolUse", "session_id": "x", "tool_name": "Bash"},
], ids=["empty", "start_no_sid", "prompt_no_sid", "pre_no_sid",
        "unknown_event", "pre_other_tool"])
def test_never_blocks(root, clock, monkeypatch, payload):
    assert _fire(payload, monkeypatch) == 0


def test_malformed_stdin_exits_zero(root, clock, monkeypatch):
    monkeypatch.setattr("sys.stdin", io.StringIO("not json at all {"))
    assert record_session_event.main() == 0


def test_no_session_recorded_when_sid_missing(root, clock, monkeypatch):
    _fire({"hook_event_name": "UserPromptSubmit", "prompt": "hi"}, monkeypatch)
    assert not os.path.isdir(os.path.join(str(root), "sessions"))
