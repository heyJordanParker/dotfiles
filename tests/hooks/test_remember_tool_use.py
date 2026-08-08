"""What an agent did, stored alongside what it said about it.

A memory built from prose alone records the agent's account of the work. These
pin which calls become a work line, which are deliberately silent, and that the
`memory: none` declaration holds here too.
"""

import io
import json
import sys

import pytest
import remember_tool_use

CFG = {
    "peerName": "jordan",
    "workspace": "parkerlabs",
    "endpoint": {"baseUrl": "https://example.invalid"},
}


@pytest.fixture
def posted(monkeypatch):
    sent = []
    monkeypatch.setattr(remember_tool_use.honcho, "config", lambda: CFG)
    monkeypatch.setattr(
        remember_tool_use.honcho, "post",
        lambda cfg, session, peer, text, created_at=None, metadata=None, timeout=None:
        sent.append((session, peer, text, timeout)) or True)
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    return sent


def _fire(monkeypatch, tool, tool_input, agent="ponytail"):
    event = {"hook_event_name": "PostToolUse", "tool_name": tool, "tool_input": tool_input,
             "cwd": "/repo", "session_id": "s1", "agent_id": "a1", "agent_type": agent}
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return remember_tool_use.main()


def test_a_write_an_edit_a_command_and_a_dispatch_each_leave_one_line(posted, monkeypatch):
    assert _fire(monkeypatch, "Write", {"file_path": "/repo/a.py"}) == 0
    assert _fire(monkeypatch, "Edit", {"file_path": "/repo/b.py"}) == 0
    assert _fire(monkeypatch, "Bash", {"command": "uv run pytest tests/hooks"}) == 0
    assert _fire(monkeypatch, "Agent", {"subagent_type": "explorer"}) == 0

    assert [text for _, _, text, _ in posted] == [
        "[work] wrote /repo/a.py",
        "[work] wrote /repo/b.py",
        "[work] ran `uv run pytest tests/hooks`",
        "[work] dispatched the explorer agent",
    ]
    assert {peer for _, peer, _, _ in posted} == {"ponytail"}
    assert {session for session, _, _, _ in posted} == {"repo"}


def test_a_read_leaves_nothing(posted, monkeypatch):
    """Reads are the bulk of every run and say only where an agent looked, which
    trace already records per file with far more precision. A read is a shell
    command as often as it is a Read tool, and `trace` most of all — our own gates
    route every read through it."""
    assert _fire(monkeypatch, "Read", {"file_path": "/repo/a.py"}) == 0
    assert _fire(monkeypatch, "Grep", {"pattern": "x"}) == 0
    for command in ("trace read packages/agents/hooks/lib/honcho.py", "git status",
                    "git log --oneline -5", "cat /repo/a.py", "cd /repo && ls",
                    "rg honcho packages/"):
        assert _fire(monkeypatch, "Bash", {"command": command}) == 0, command
    assert posted == []

    # A command that changes something is still work.
    assert _fire(monkeypatch, "Bash", {"command": "git commit -m 'fix the parser'"}) == 0
    assert len(posted) == 1


def test_a_long_command_is_stored_by_its_shape(posted, monkeypatch):
    """A heredoc or a whole script turns a work record into a paste bin."""
    assert _fire(monkeypatch, "Bash", {"command": "uv run python -c '" + "x" * 500 + "'"}) == 0
    assert len(posted[0][2]) <= len("[work] ran ``") + remember_tool_use.COMMAND_LIMIT


def test_the_write_is_capped_below_the_one_nobody_waits_on(posted, monkeypatch):
    """This write sits in front of the tool result the agent is waiting on, so a
    slow server costs the agent the wait on every single tool call."""
    assert _fire(monkeypatch, "Write", {"file_path": "/repo/a.py"}) == 0
    assert posted[0][3] == remember_tool_use.POST_TIMEOUT < remember_tool_use.honcho.TIMEOUT


def test_an_agent_declaring_no_memory_leaves_nothing(posted, monkeypatch, tmp_path):
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    (tmp_path / "agents").mkdir()
    (tmp_path / "agents" / "researcher.md").write_text(
        "---\nname: researcher\nmemory: none\n---\n")
    assert _fire(monkeypatch, "Write", {"file_path": "/repo/a.py"}, agent="researcher") == 0
    assert posted == []


def test_a_call_with_no_agent_behind_it_leaves_nothing(posted, monkeypatch):
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(
        {"hook_event_name": "PostToolUse", "tool_name": "Write",
         "tool_input": {"file_path": "/repo/a.py"}, "cwd": "/repo", "session_id": "s1"})))
    assert remember_tool_use.main() == 0
    assert posted == []
