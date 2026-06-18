"""Coverage for record_session_event.py — the one hook that reads a Claude Code
event and records the matching session-state change through the spine.

It centralizes the session-state recording that settings.json wires for each
Claude Code event. The tests feed real event
payloads and assert the resulting on-disk state (state.json / reads.jsonl /
skills.jsonl) is exactly what the spine produces today, plus the agent-id rewrite,
the completed-task-notification stop+archive path, and the never-block contract.

The store is isolated under tmp_path via CLAUDE_DATA_ROOT (same as the spine
tests). The hook imports the spine as `lib.session_state` and the archive as
`archive_subagent_log`; we drive the hook through its `main()` by feeding the
payload on stdin, and monkeypatch the spine's two clock functions for determinism.
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

    def _iso():
        return "1970-01-01T00:00:%02dZ" % (state["now"] % 60)

    monkeypatch.setattr(session_state, "_now", lambda: state["now"])
    monkeypatch.setattr(session_state, "_iso_now", _iso)
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


def _logfile(root, sid, name):
    return os.path.join(str(root), "sessions", sid, name)


# ---------------------------------------------------------------------------
# The seven recording commands: event in -> spine result out
# ---------------------------------------------------------------------------

def test_session_start_records_start(root, clock, monkeypatch):
    clock(4242)
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "s1",
                "transcript_path": "/p/s1.jsonl"}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "s1"))
    assert st["role"] == "main"
    assert st["session_id"] == "s1"
    assert st["session_start"] == 4242
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
    st = _read(_state(root, "s2"))
    assert st["human_turns"] == 1
    assert st["current_turn_start"] == 1100


def test_user_prompt_filters_system_injected(root, clock, monkeypatch):
    """The spine drops system-injected prompts; the hook must hand it the raw text
    so that filtering happens — a bracket-enclosed single line is system-injected."""
    _fire({"hook_event_name": "SessionStart", "session_id": "s3",
           "transcript_path": "/p/s3.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "s3",
                "prompt": "[system reminder]"}, monkeypatch)
    assert rc == 0
    st = _read(_state(root, "s3"))
    assert st["human_turns"] == 0
    assert st["current_turn_start"] is None


def test_stop_records_stopped(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s4",
           "transcript_path": "/p/s4.jsonl"}, monkeypatch)
    clock(5000)
    rc = _fire({"hook_event_name": "Stop", "session_id": "s4"}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "s4"))["last_stop"] == 5000


def test_post_tool_use_increments_tool_count(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s5",
           "transcript_path": "/p/s5.jsonl"}, monkeypatch)
    for _ in range(3):
        rc = _fire({"hook_event_name": "PostToolUse", "session_id": "s5",
                    "tool_name": "Bash"}, monkeypatch)
        assert rc == 0
    assert _read(_state(root, "s5"))["tools_used"] == 3


def test_pre_tool_use_read_records_path(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s6",
           "transcript_path": "/p/s6.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "s6",
                "tool_name": "Read",
                "tool_input": {"file_path": "/some/file.ts"}}, monkeypatch)
    assert rc == 0
    with open(_logfile(root, "s6", "reads.jsonl")) as fh:
        entry = json.loads(fh.readline())
    assert entry["path"] == "/some/file.ts"


def test_pre_tool_use_read_without_path_skips(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s7",
           "transcript_path": "/p/s7.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "s7",
                "tool_name": "Read", "tool_input": {}}, monkeypatch)
    assert rc == 0
    assert not os.path.isfile(_logfile(root, "s7", "reads.jsonl"))


def test_pre_tool_use_skill_records_name(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s8",
           "transcript_path": "/p/s8.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "s8",
                "tool_name": "Skill",
                "tool_input": {"skill": "naming"}}, monkeypatch)
    assert rc == 0
    with open(_logfile(root, "s8", "skills.jsonl")) as fh:
        entry = json.loads(fh.readline())
    assert entry["skill"] == "naming"


@pytest.mark.parametrize("key", ["skill", "skill_name", "name"])
def test_pre_tool_use_skill_accepts_each_payload_key(root, clock, monkeypatch, key):
    """The retired jq tried .skill // .skill_name // .name — match all three."""
    sid = "sk-%s" % key
    _fire({"hook_event_name": "SessionStart", "session_id": sid,
           "transcript_path": "/p/%s.jsonl" % sid}, monkeypatch)
    _fire({"hook_event_name": "PreToolUse", "session_id": sid,
           "tool_name": "Skill", "tool_input": {key: "trace"}}, monkeypatch)
    with open(_logfile(root, sid, "skills.jsonl")) as fh:
        assert json.loads(fh.readline())["skill"] == "trace"


def test_pre_tool_use_skill_without_name_skips(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s9",
           "transcript_path": "/p/s9.jsonl"}, monkeypatch)
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "s9",
                "tool_name": "Skill", "tool_input": {}}, monkeypatch)
    assert rc == 0
    assert not os.path.isfile(_logfile(root, "s9", "skills.jsonl"))


def test_post_compact_truncates_logs(root, clock, monkeypatch):
    _fire({"hook_event_name": "SessionStart", "session_id": "s10",
           "transcript_path": "/p/s10.jsonl"}, monkeypatch)
    _fire({"hook_event_name": "PreToolUse", "session_id": "s10", "tool_name": "Read",
           "tool_input": {"file_path": "/a"}}, monkeypatch)
    _fire({"hook_event_name": "PreToolUse", "session_id": "s10", "tool_name": "Skill",
           "tool_input": {"skill": "trace"}}, monkeypatch)
    rc = _fire({"hook_event_name": "PostCompact", "session_id": "s10"}, monkeypatch)
    assert rc == 0
    assert os.path.getsize(_logfile(root, "s10", "reads.jsonl")) == 0
    assert os.path.getsize(_logfile(root, "s10", "skills.jsonl")) == 0


# ---------------------------------------------------------------------------
# Agent-id rewrite: subagent per-tool-call events nest under the parent
# ---------------------------------------------------------------------------

def _start_parent_and_subagent(root, monkeypatch, parent, agent_task):
    """Establish a parent main session and a subagent under it via the spine, so
    the nested session dir resolves for the per-tool-call recordings."""
    _fire({"hook_event_name": "SessionStart", "session_id": parent,
           "transcript_path": "/p/%s/%s.jsonl" % (parent, parent)}, monkeypatch)
    session_state.cmd_start(["agent-%s" % agent_task, "--transcript-path",
                             "/p/%s/subagents/agent-%s.jsonl" % (parent, agent_task)])


def test_tool_used_with_agent_id_nests_under_parent(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "par1", "abc")
    rc = _fire({"hook_event_name": "PostToolUse", "session_id": "par1",
                "agent_id": "abc", "tool_name": "Bash"}, monkeypatch)
    assert rc == 0
    nested = _state(root, "par1", "subagents", "agent-abc")
    assert os.path.isfile(nested)
    st = _read(nested)
    assert st["parent_session_id"] == "par1"
    assert st["tools_used"] == 1
    # parent's own counter untouched
    assert _read(_state(root, "par1"))["tools_used"] == 0


def test_read_with_agent_id_nests_under_parent(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "par2", "def")
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "par2",
                "agent_id": "def", "tool_name": "Read",
                "tool_input": {"file_path": "/x.ts"}}, monkeypatch)
    assert rc == 0
    nested_log = os.path.join(str(root), "sessions", "par2", "subagents",
                              "agent-def", "reads.jsonl")
    assert os.path.isfile(nested_log)
    with open(nested_log) as fh:
        assert json.loads(fh.readline())["path"] == "/x.ts"
    assert not os.path.isfile(_logfile(root, "par2", "reads.jsonl"))


def test_skill_with_agent_id_nests_under_parent(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "par3", "ghi")
    rc = _fire({"hook_event_name": "PreToolUse", "session_id": "par3",
                "agent_id": "ghi", "tool_name": "Skill",
                "tool_input": {"skill": "subagents"}}, monkeypatch)
    assert rc == 0
    nested_log = os.path.join(str(root), "sessions", "par3", "subagents",
                              "agent-ghi", "skills.jsonl")
    with open(nested_log) as fh:
        assert json.loads(fh.readline())["skill"] == "subagents"


def test_start_does_not_apply_agent_rewrite(root, clock, monkeypatch):
    """SessionStart records against the plain session id even when agent_id is
    present — the rewrite is per-tool-call only, matching the old wiring."""
    rc = _fire({"hook_event_name": "SessionStart", "session_id": "plain1",
                "agent_id": "zzz", "transcript_path": "/p/plain1.jsonl"}, monkeypatch)
    assert rc == 0
    assert os.path.isfile(_state(root, "plain1"))
    assert not os.path.isdir(os.path.join(str(root), "sessions", "plain1", "subagents"))


# ---------------------------------------------------------------------------
# task-notification: completed -> subagent stop + log archive; else nothing
# ---------------------------------------------------------------------------

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


def test_completed_notification_stops_subagent_and_archives(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "pn1", "feed")
    active, archived = _seed_tracer_log(monkeypatch, "pn1", "feed")
    clock(7000)
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "pn1",
                "prompt": _notification("feed", "completed")}, monkeypatch)
    assert rc == 0
    # subagent stop recorded
    nested = _state(root, "pn1", "subagents", "agent-feed")
    assert _read(nested)["last_stop"] == 7000
    # tracer log archived (active gone, archived present)
    assert not os.path.isdir(active)
    assert os.path.isdir(archived)


def test_noncompleted_notification_does_nothing(root, clock, monkeypatch):
    _start_parent_and_subagent(root, monkeypatch, "pn2", "beef")
    active, archived = _seed_tracer_log(monkeypatch, "pn2", "beef")
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "pn2",
                "prompt": _notification("beef", "failed")}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "pn2", "subagents", "agent-beef"))["last_stop"] is None
    assert os.path.isdir(active)
    assert not os.path.isdir(archived)


def test_plain_prompt_is_not_a_notification(root, clock, monkeypatch):
    """A normal human prompt records a turn and triggers no archive/stop."""
    _start_parent_and_subagent(root, monkeypatch, "pn3", "cafe")
    active, archived = _seed_tracer_log(monkeypatch, "pn3", "cafe")
    rc = _fire({"hook_event_name": "UserPromptSubmit", "session_id": "pn3",
                "prompt": "regular human prompt"}, monkeypatch)
    assert rc == 0
    assert _read(_state(root, "pn3"))["human_turns"] == 1
    assert os.path.isdir(active)
    assert not os.path.isdir(archived)


# ---------------------------------------------------------------------------
# Never-block contract: exit 0 on every degenerate path
# ---------------------------------------------------------------------------

@pytest.mark.parametrize("payload", [
    {},
    {"hook_event_name": "SessionStart"},                       # no session id
    {"hook_event_name": "UserPromptSubmit", "prompt": "hi"},   # no session id
    {"hook_event_name": "Stop"},                               # no session id
    {"hook_event_name": "PostToolUse", "tool_name": "Bash"},   # no session id
    {"hook_event_name": "PreToolUse", "tool_name": "Read",
     "tool_input": {"file_path": "/x"}},                       # no session id
    {"hook_event_name": "PostCompact"},                        # no session id
    {"hook_event_name": "SomethingUnknown", "session_id": "x"},
    {"hook_event_name": "PreToolUse", "session_id": "x", "tool_name": "Bash"},  # PreToolUse non-read/skill
], ids=["empty", "start_no_sid", "prompt_no_sid", "stop_no_sid", "post_no_sid",
        "pre_no_sid", "compact_no_sid", "unknown_event", "pre_other_tool"])
def test_never_blocks(root, clock, monkeypatch, payload):
    assert _fire(payload, monkeypatch) == 0


def test_malformed_stdin_exits_zero(root, clock, monkeypatch):
    monkeypatch.setattr("sys.stdin", io.StringIO("not json at all {"))
    assert record_session_event.main() == 0


def test_no_session_recorded_when_sid_missing(root, clock, monkeypatch):
    _fire({"hook_event_name": "PostToolUse", "tool_name": "Bash"}, monkeypatch)
    assert not os.path.isdir(os.path.join(str(root), "sessions"))
