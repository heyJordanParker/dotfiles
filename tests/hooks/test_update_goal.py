"""Behavioral tests for update_goal.py — the goal/requirements/boundaries hook.

The LLM call and memory recall are stubbed, so these pin the deterministic shell
around them: the structural skips, the hard 10-item list cap, the spine write, and
the message built back from the spine (goal/requirements/boundaries from state,
skills + take + optional note from the model).
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
    monkeypatch.setattr(update_goal, "recall_memory", lambda sid: "")
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


def test_list_cap_enforced(monkeypatch, spine_root):
    reqs = ["r%d" % i for i in range(15)]
    bnds = ["b%d" % i for i in range(12)]
    rc, text = _run(monkeypatch,
                    {"session_id": "ug1", "prompt": "do x"},
                    {"goal": "Do x.", "requirements": reqs, "boundaries": bnds})
    assert rc == 0
    st = load_state("ug1")
    assert st["goal"] == "Do x."
    assert st["requirements"] == reqs[:10]
    assert st["boundaries"] == bnds[:10]


def test_message_emits_goal_skills_note_not_lists(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ug1", "prompt": "do x /cc"},
                   {"goal": "Do x.", "requirements": ["must y"], "boundaries": ["never z"],
                    "skills": ["/cc"], "note": "heads up"})
    assert "Session goal:\nDo x." in text
    # requirements/boundaries stay on the spine but are NOT shown to the main agent
    assert "Requirements:" not in text and "must y" not in text
    assert "Boundaries:" not in text and "never z" not in text
    assert "Skills to execute: /cc" in text
    assert text.endswith("heads up")


def test_note_omitted_when_absent(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ug1", "prompt": "do x"},
                   {"goal": "Do x.", "requirements": [], "boundaries": []})
    assert "restating what the architect asked" in text  # every-turn opener directive
    assert text.endswith("Session goal:\nDo x.")          # goal last, no note appended


def test_carries_goal_forward_when_model_omits_lists(monkeypatch, spine_root):
    _run(monkeypatch, {"session_id": "ug1", "prompt": "first"},
         {"goal": "G1.", "requirements": ["a"], "boundaries": ["b"]})
    # A later turn whose model result omits the arrays must not wipe the spine.
    _, text = _run(monkeypatch, {"session_id": "ug1", "prompt": "second"}, {"goal": "G2."})
    st = load_state("ug1")
    assert st["goal"] == "G2."
    assert st["requirements"] == ["a"]
    assert st["boundaries"] == ["b"]


def test_take_emitted_with_inference_framing(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ug1", "prompt": "no, narrow it to X"},
                   {"goal": "Do x.", "requirements": [], "boundaries": [],
                    "take": "The user is correcting the agent toward narrowing the boundary."})
    assert "The user is correcting the agent toward narrowing the boundary." in text
    # framed as the hook's inference, not the architect's own words
    assert "inference" in text and "not the architect's words" in text
    # the take is the last line — the freshest signal for the main agent
    assert text.rstrip().endswith("narrowing the boundary.")
