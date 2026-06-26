"""Behavioral tests for classify_intent.py — intent → contract injection + notes.

The LLM call is stubbed, so these pin the deterministic shell around it: the
per-intent contract emitted as additionalContext, the question-with-action
variant, the sequential directive, session-notes maintenance (capped at 10), the
proposing-turn re-injection of standing notes, and composition with a typed
mode-command. The structural skips and the LLM-down typed-command fallback are
covered in test_local_llm_fallbacks.
"""

import classify_intent
import pytest
from lib.session_state import load_state, merge_state


@pytest.fixture
def spine_root(tmp_path, monkeypatch):
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    return root


def _run(monkeypatch, payload, model_result):
    monkeypatch.setattr(classify_intent, "run_model", lambda *a, **k: model_result)
    monkeypatch.setattr(classify_intent, "read_event", lambda: payload)
    captured = {}
    monkeypatch.setattr(classify_intent, "emit_context",
                        lambda text: captured.setdefault("text", text))
    rc = classify_intent.main()
    return rc, captured.get("text")


def test_question_emits_answer_contract(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "why does X work this way?"},
                    {"intent": "question"})
    assert rc == 0
    assert "This is a question. Answer it" in text
    assert "Answer with specific facts, not gestures at them" in text
    assert "execute the action items" not in text


def test_question_with_action_items(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "how does X work? also change Y"},
                   {"intent": "question", "has_action_items": True})
    assert "Answer the question first, then execute the action items" in text
    assert "Answer with specific facts, not gestures at them" in text


def test_correction_emits_same_format_contract(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "no, that's wrong, use Y"},
                   {"intent": "correction"})
    assert "re-deliver the whole response in the same format as the original" in text
    assert "never a prose diff" in text


def test_plain_action_emits_only_standing_reminders(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "add a guard to the parser"},
                    {"intent": "action"})
    assert rc == 0
    assert "The architect's call governs" in text
    assert "Ground every claim" in text
    assert "This is a question" not in text


def test_sequential_directive_appended(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "do A, then B, finally C"},
                   {"intent": "action", "sequential": True})
    assert "strictly sequential" in text


def test_notes_persisted_and_capped(monkeypatch, spine_root):
    notes = ["note %d" % i for i in range(15)]
    _run(monkeypatch, {"session_id": "ci1", "prompt": "you broke the build again"},
         {"intent": "correction", "notes": notes})
    assert load_state("ci1")["notes"] == notes[:10]


def test_notes_reinjected_on_proposing_turn(monkeypatch, spine_root):
    # A fresh main session defaults to the proposing state.
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "look into the auth flow"},
                   {"intent": "action", "notes": ["never touch the tmux config"]})
    assert "Past corrections from this session — do not re-violate:" in text
    assert "never touch the tmux config" in text


def test_notes_not_reinjected_off_proposing(monkeypatch, spine_root):
    merge_state("ci1", {"state": "executing"})
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "implement the slice"},
                   {"intent": "action", "notes": ["never touch the tmux config"]})
    # Persisted, but not surfaced outside a proposing turn.
    assert load_state("ci1")["notes"] == ["never touch the tmux config"]
    assert "never touch the tmux config" not in text
    # The standing reminders still ride every turn.
    assert "The architect's call governs" in text


def test_banned_phrases_captured_to_spine(monkeypatch, spine_root):
    _run(monkeypatch, {"session_id": "ci1", "prompt": "stop saying actor"},
         {"intent": "correction", "banned_phrases": ["actor"]})
    assert "actor" in load_state("ci1")["banned_phrases"]


def test_banned_phrases_dedup_case_insensitive(monkeypatch, spine_root):
    merge_state("ci1", {"banned_phrases": ["actor"]})
    _run(monkeypatch, {"session_id": "ci1", "prompt": "stop saying Actor"},
         {"intent": "correction", "banned_phrases": ["Actor"]})
    assert load_state("ci1")["banned_phrases"] == ["actor"]


def test_typed_propose_composes_with_question_contract(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "/propose what about caching X?"},
                   {"intent": "question"})
    assert "This is a proposing-state turn. Load the /propose skill now" in text
    assert "This is a question. Answer it" in text


def test_standing_reminders_present_and_not_duplicated(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "why does X work this way?"},
                   {"intent": "question"})
    # All three reminders ride every turn.
    assert "The architect's call governs" in text
    assert "Ground every claim and recommendation in the code" in text
    assert "Don't cut research short" in text
    # The generic no-sycophant line moved to the standing block, not duplicated
    # back into the question contract.
    assert "Don't be a sycophant" not in text
