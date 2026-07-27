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


@pytest.mark.parametrize("subagent_type", ["Explore", "general-purpose", "Plan",
                                           "claude-code-guide"])
def test_builtin_types_are_redirected(monkeypatch, subagent_type, capsys):
    """Each built-in has a specialized replacement; the block names which one."""
    assert _run(monkeypatch, {"subagent_type": subagent_type}) == 2
    assert "BLOCKED" in _feedback(capsys)


def test_custom_type_passes(monkeypatch):
    assert _run(monkeypatch, {"subagent_type": "backend-engineer"}) == 0


@pytest.mark.parametrize("model", ["sonnet", "haiku"])
def test_cheap_model_override_is_blocked(monkeypatch, model, capsys):
    """Agents declare their model in frontmatter; overriding down defeats it."""
    assert _run(monkeypatch, {"subagent_type": "ponytail", "model": model}) == 2
    assert model in _feedback(capsys)


@pytest.mark.parametrize("model", ["opus", "fable"])
def test_allowed_model_overrides_pass(monkeypatch, model):
    assert _run(monkeypatch, {"subagent_type": "ponytail", "model": model}) == 0


def test_no_model_override_passes(monkeypatch):
    assert _run(monkeypatch, {"subagent_type": "ponytail"}) == 0


def test_name_is_blocked_before_the_model_check(monkeypatch, capsys):
    """A dispatch that is both named and downgraded reports the name, so fixing
    the model alone does not leave the report channel broken."""
    assert _run(monkeypatch, {"subagent_type": "ponytail", "name": "w",
                              "model": "sonnet"}) == 2
    assert "teammate" in _feedback(capsys)
