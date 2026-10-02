"""The per-agent memory gate: one `memory: none` declaration, both harnesses."""

import io
import json
import sys

import block_memory_access
import pytest

BLANK = "---\nname: %s\nmodel: opus\nmemory: none\n---\n\nbody\n"
DECLARED_ON = "---\nname: %s\nmodel: opus\nmemory: user\n---\n\nbody\n"
UNDECLARED = "---\nname: %s\nmodel: opus\n---\n\nbody\n"


@pytest.fixture
def config_root(tmp_path, monkeypatch):
    """A Claude config root with an agents/ dir; returns a writer for it."""
    agents = tmp_path / "agents"
    agents.mkdir()
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))

    def write(name, template):
        (agents / (name + ".md")).write_text(template % name)
    return write


def _run(monkeypatch, agent_type, command="honcho context jordan",
         agent_id="a1d6344fe2b8d8332"):
    """A subagent's PreToolUse payload by default: both fields present, which is
    the shape observed from a real Agent-tool dispatch. `agent_id=None` is the
    main thread, which carries `agent_type` alone when started with --agent."""
    event = {"hook_event_name": "PreToolUse", "tool_name": "Bash",
             "tool_input": {"command": command}}
    if agent_type is not None:
        event["agent_type"] = agent_type
    if agent_id is not None:
        event["agent_id"] = agent_id
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return block_memory_access.main()








def test_declared_blank_agent_is_refused(config_root, monkeypatch, capsys):
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher") == 2
    assert "researcher agent declares `memory: none`" in capsys.readouterr().err


def test_agent_declaring_memory_on_reaches_memory(config_root, monkeypatch):
    config_root("architect", DECLARED_ON)
    assert _run(monkeypatch, "architect") == 0








def test_main_session_started_with_the_agent_keeps_memory(config_root, monkeypatch):
    """The pair that defines the gate. `agent_type` alone is the main thread of a
    session started with `--agent` — the architect's shell alias starts every
    session that way, so keying the presence test on it would take memory off his
    own conversation the moment he launched one as a blank-declaring agent.
    Observed live: a `--agent blankprobe` main session carries
    `{"agent_id": null, "agent_type": "blankprobe"}`."""
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher", agent_id=None) == 0


def test_subagent_of_that_same_agent_is_refused(config_root, monkeypatch, capsys):
    """The other half: the same declaration, the same name, inside a subagent —
    `agent_id` present — is refused. Observed live: an Agent-tool dispatch of the
    same agent carries `{"agent_id": "a1d6…", "agent_type": "blankprobe"}`."""
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher") == 2
    assert "researcher agent declares `memory: none`" in capsys.readouterr().err















# --- the declaration fails closed, never open --------------------------------------





