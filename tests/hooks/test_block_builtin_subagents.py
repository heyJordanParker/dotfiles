"""The Agent-dispatch gate: which subagent_type, which model, and no teammates."""

import io
import json
import sys

import block_builtin_subagents
import pytest


def _run(monkeypatch, tool_input):
    event = {"hook_event_name": "PreToolUse", "tool_name": "Agent",
             "tool_input": tool_input}
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return block_builtin_subagents.main()


def _feedback(capsys):
    """The block text, wherever the feedback layer put it. Drains the buffer, so
    each test reads it once."""
    captured = capsys.readouterr()
    return captured.out + captured.err


def test_named_dispatch_is_blocked(monkeypatch, capsys):
    """A name makes the dispatch a teammate, and a teammate's report reaches the
    dispatcher only if it calls SendMessage itself — reports get lost that way."""
    assert _run(monkeypatch, {"subagent_type": "ponytail", "name": "worker"}) == 2
    out = _feedback(capsys)
    assert "teammate" in out and "agentId" in out


def test_unnamed_dispatch_passes(monkeypatch):
    assert _run(monkeypatch, {"subagent_type": "ponytail", "prompt": "do x"}) == 0


@pytest.mark.parametrize("subagent_type", ["Explore"])
def test_builtin_types_are_redirected(monkeypatch, subagent_type, capsys):
    """Each built-in has a specialized replacement; the block names which one."""
    assert _run(monkeypatch, {"subagent_type": subagent_type}) == 2
    output = _feedback(capsys)
    assert "BLOCKED" in output
    if subagent_type == "general-purpose":
        assert "\n  codex " not in output


def test_custom_type_passes(monkeypatch):
    assert _run(monkeypatch, {"subagent_type": "backend-engineer"}) == 0


@pytest.mark.parametrize("model", ["sonnet"])
def test_non_opus_model_override_is_blocked(monkeypatch, model, capsys):
    """Agents declare their model in frontmatter; overriding defeats it. Fable is
    blocked with the cheap models, for the opposite reason: it bills twice opus
    for the same tokens, and a dispatch pays that on the agent's whole context
    before it does any work."""
    assert _run(monkeypatch, {"subagent_type": "ponytail", "model": model}) == 2
    assert model in _feedback(capsys)


def test_opus_override_passes(monkeypatch):
    assert _run(monkeypatch, {"subagent_type": "ponytail", "model": "opus"}) == 0



