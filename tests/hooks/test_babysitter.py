"""Behavioral tests for babysitter.py — the human-optimization stop gate.

The LLM verdict is stubbed, so these pin the deterministic shell around it: the
agent-session skip, the empty-message skip, the allow path (exit 0, silent) and
the concern path (exit 0, the model's reason surfaced as a systemMessage on
stdout via feedback.raise_concern), the fire-once-then-yield cap, and that the
eval prompt carries the architect's checks.
"""

import babysitter
import pytest


@pytest.fixture
def spine_root(tmp_path, monkeypatch):
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    return root


def _run(monkeypatch, capsys, payload, model_result):
    monkeypatch.setattr(babysitter, "run_model", lambda *a, **k: model_result)
    monkeypatch.setattr(babysitter, "read_event", lambda: payload)
    rc = babysitter.main()
    captured = capsys.readouterr()
    return rc, captured.out, captured.err


def _payload(prompt_msg, sid="bs1"):
    return {"session_id": sid, "last_assistant_message": prompt_msg,
            "transcript_path": "", "cwd": ""}


def test_allows_when_model_allows(monkeypatch, spine_root, capsys):
    rc, out, err = _run(monkeypatch, capsys, _payload("here is a concrete proposal"),
                        {"allow": True})
    assert rc == 0
    assert out == "" and err == ""


def test_blocks_when_model_blocks(monkeypatch, spine_root, capsys):
    # A concern is non-halting: exit 0 with the model's reason wrapped in a
    # systemMessage on stdout, not a stderr block.
    rc, out, err = _run(monkeypatch, capsys, _payload("we should add a backend factory"),
                        {"allow": False, "reason": "ABSTRACTION-NOT-CONCRETE: 'backend factory'"})
    assert rc == 0
    assert "ABSTRACTION-NOT-CONCRETE" in out
    assert err == ""


def test_empty_message_allows(monkeypatch, spine_root, capsys):
    rc, _, _ = _run(monkeypatch, capsys, _payload("   "), {"allow": False})
    assert rc == 0


def test_blocks_once_then_yields_within_a_turn(monkeypatch, spine_root, capsys):
    # The model says block both times; the gate fires on the first stop, records
    # it, and yields the second — the loop-break. Driving main() twice (not a
    # manual bump) is what proves the concern path actually records the block.
    block = {"allow": False, "reason": "x"}
    rc1, out1, _ = _run(monkeypatch, capsys, _payload("not optimized"), block)
    assert rc1 == 0 and "Potential issue" in out1
    rc2, out2, _ = _run(monkeypatch, capsys, _payload("still not optimized"), block)
    assert rc2 == 0 and out2 == ""


def test_awaiting_background_subagents_allows(monkeypatch, spine_root, capsys,
                                              write_transcript):
    # The turn dispatched a background subagent and the agent is pausing to await
    # it — an async wait, not skipped work. Allowed even though the model blocks.
    tpath = write_transcript([
        {"type": "user", "message": {"content": "do it"}},
        {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "Agent", "id": "1",
             "input": {"run_in_background": True, "prompt": "go"}}]}},
    ])
    payload = {"session_id": "bs1", "transcript_path": tpath, "cwd": "",
               "last_assistant_message": "dispatched the agent, awaiting its result"}
    rc, _, _ = _run(monkeypatch, capsys, payload,
                    {"allow": False, "reason": "x"})
    assert rc == 0


def test_agent_session_is_skipped(monkeypatch, spine_root, capsys):
    rc, _, _ = _run(monkeypatch, capsys, _payload("anything", sid="agent-x"),
                    {"allow": False})
    assert rc == 0


def test_model_unavailable_does_not_block(monkeypatch, spine_root, capsys):
    rc, _, _ = _run(monkeypatch, capsys, _payload("anything"), None)
    assert rc == 0


def test_eval_prompt_carries_all_checks():
    prompt = babysitter._eval_prompt("the request", "the reply", "proposing")
    for rule in (
        "1. UNDEFINED AGENT COINAGE", "2. INCONSISTENT-TERMS", "3. AMBIGUOUS",
        "4. ABSTRACTION-NOT-CONCRETE", "5. NOT-STRUCTURED", "6. CHAIN-OF-THOUGHT-DUMP",
        "7. WASTES-TIME", "8. PADDED-OPTIONS", "9. SCOPE-TAMPERING",
        "10. REQUIRES-MEMORY",
    ):
        assert rule in prompt
    assert "UserFactory.php" in prompt  # the concrete-over-abstraction example


def test_eval_prompt_reflects_mode_off_state():
    # The turn's mode keys on state: a proposing turn is alignment, where surfacing
    # a decision is the job — never scope-tampering.
    assert "Session state: proposing" in babysitter._eval_prompt("q", "a", "proposing")
    assert "Session state: executing" in babysitter._eval_prompt("q", "a", "executing")
