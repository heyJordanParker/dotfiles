"""Behavioral tests for update_goal.py — the session-goal hook.

The LLM call is stubbed, so these pin the deterministic shell around it: the
structural skips, the spine write, and the message built back from the spine
(goal from state, take + optional note from the model).
"""

import pytest

import update_goal
from lib.session_state import load_state


@pytest.fixture
def spine_root(tmp_path, monkeypatch):
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    return root


def _run(monkeypatch, payload, model_result):
    monkeypatch.setattr(update_goal, "run_model", lambda *a, **k: model_result)
    monkeypatch.setattr(update_goal, "read_event", lambda: payload)
    captured = {}
    monkeypatch.setattr(update_goal, "emit_context",
                        lambda text: captured.setdefault("text", text))
    rc = update_goal.main()
    return rc, captured.get("text")


def test_skips_subagent_session(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "agent-x", "prompt": "build it"}, {"goal": "x"})
    assert rc == 0 and text is None
    assert load_state("agent-x") == {}


def test_skips_system_prompt(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ug1", "prompt": "<task>x</task>"}, {"goal": "x"})
    assert rc == 0 and text is None


def test_llm_down_writes_no_goal(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ug1", "prompt": "build a thing"}, None)
    assert rc == 0 and text is None
    assert load_state("ug1").get("goal") is None


def test_goal_written_to_spine(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ug1", "prompt": "do x"}, {"goal": "Do x."})
    assert rc == 0
    assert load_state("ug1")["goal"] == "Do x."


def test_message_emits_goal_and_note(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ug1", "prompt": "do x"},
                   {"goal": "Do x.", "note": "heads up"})
    assert "Session goal:\nDo x." in text
    assert text.endswith("heads up")


def test_no_rendered_opening_demanded(monkeypatch, spine_root):
    """The hook injects the goal as context; it never tells the agent to render it."""
    _, text = _run(monkeypatch,
                   {"session_id": "ug1", "prompt": "do x"}, {"goal": "Do x."})
    assert text == "Session goal:\nDo x."       # goal only, no directive, no note


def test_goal_updates_across_turns(monkeypatch, spine_root):
    _run(monkeypatch, {"session_id": "ug1", "prompt": "first"}, {"goal": "G1."})
    _run(monkeypatch, {"session_id": "ug1", "prompt": "second"}, {"goal": "G2."})
    assert load_state("ug1")["goal"] == "G2."


def test_goal_survives_a_turn_the_model_omits_it(monkeypatch, spine_root):
    _run(monkeypatch, {"session_id": "ug1", "prompt": "first"}, {"goal": "G1."})
    _run(monkeypatch, {"session_id": "ug1", "prompt": "second"}, {"take": "asking a question"})
    assert load_state("ug1")["goal"] == "G1."


def test_take_emitted_with_inference_framing(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ug1", "prompt": "no, narrow it to X"},
                   {"goal": "Do x.",
                    "take": "The user is correcting the agent toward narrowing the boundary."})
    assert "The user is correcting the agent toward narrowing the boundary." in text
    # framed as the hook's inference, not the architect's own words
    assert "inference" in text and "not the architect's words" in text
    # the take is the last line — the freshest signal for the main agent
    assert text.rstrip().endswith("narrowing the boundary.")
