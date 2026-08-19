"""Behavioral tests for babysitter.py — the one stop gate.

The LLM verdict is stubbed, so these pin the deterministic shell around it: the
agent-session skip, the allow path (exit 0, silent) and the concern path (exit 0,
the model's reason surfaced as a systemMessage on stdout via
feedback.raise_concern), plus the facts that decide which Rules the call carries
at all. The verdict itself is judged by replaying real moments through
tests/hooks/scenarios/.
"""

import babysitter
import pytest


@pytest.fixture
def state_root(tmp_path, monkeypatch):
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


def test_allows_when_model_allows(monkeypatch, state_root, capsys):
    rc, out, err = _run(monkeypatch, capsys, _payload("here is a concrete proposal"),
                        {"allow": True})
    assert rc == 0
    assert out == "" and err == ""


def test_blocks_when_model_blocks(monkeypatch, state_root, capsys):
    # A concern is non-halting: exit 0 with the model's reason wrapped in a
    # systemMessage on stdout, not a stderr block.
    rc, out, err = _run(monkeypatch, capsys, _payload("we should add a backend factory"),
                        {"allow": False, "reason": "ABSTRACTION-NOT-CONCRETE: 'backend factory'"})
    assert rc == 0
    assert "ABSTRACTION-NOT-CONCRETE" in out
    assert err == ""








def test_agent_session_is_skipped(monkeypatch, state_root, capsys):
    rc, _, _ = _run(monkeypatch, capsys, _payload("anything", sid="agent-x"),
                    {"allow": False})
    assert rc == 0


# --- which Rules the turn admits -------------------------------------------


def _titles(*args, **kwargs):
    return [r.split("\n")[0] for r in babysitter._rules(*args, **kwargs)]


def test_plan_exit_ignores_the_word_in_turn_text():
    """A mention of ExitPlanMode is not a plan exit.

    The check was a substring scan over the turn's raw JSONL, and this gate's own
    rendered prompt carries the words. That text lands back in the transcript, so
    the scan reported an approved plan that never happened and, beside a
    permission phrase, emitted the approved-plan concern with no model call."""
    turn = [{"type": "assistant", "message": {"content": [
        {"type": "text", "text": "ExitPlanMode in current turn: false"},
        {"type": "tool_use", "name": "Bash", "input": {"command": "ls"}},
    ]}}]
    assert babysitter._exited_plan_mode(turn) is False


def test_plan_exit_sees_the_real_tool_call():
    turn = [{"type": "assistant", "message": {"content": [
        {"type": "tool_use", "name": "ExitPlanMode", "input": {"plan": "# Plan"}},
    ]}}]
    assert babysitter._exited_plan_mode(turn) is True


def test_reading_rules_stay_out_without_the_read_log():
    """Absent the opened-files list, "not in the list" is true of every file in the
    repo. A measured control run showed the reading Rules blocking 3 of 5 sound
    proposals on exactly that, so they are admitted only with the log present."""
    without = _titles("action", "execute", "build", True, False, False)
    assert not [t for t in without if "never opened" in t or "precedent" in t]
    with_log = _titles("action", "execute", "build", True, True, False)
    assert [t for t in with_log if "never opened" in t]


def test_approved_work_rules_stay_out_of_a_correction():
    """He rejects an option, the agent re-proposes and asks for his call. The state
    axis can still read execute from earlier work, and judging approval from the
    message text is what made that read as work already handed over."""
    correction = _titles("correction", "execute", "build")
    assert not [t for t in correction if "permission" in t or "deferred" in t]
    assert [t for t in _titles("action", "execute", "build")
            if t.startswith("### Name a request for permission")]


def test_proposal_rules_follow_the_state_axis():
    assert [t for t in _titles("action", "propose", "build") if "proposal failure" in t]
    assert not [t for t in _titles("action", "execute", "build") if "proposal failure" in t]


def test_missing_paths_names_only_what_does_not_resolve(tmp_path):
    """A file the message proposes creating can never be in the opened set, so
    without this the unopened-edit Rule reads a new file as code changed blind."""
    (tmp_path / "real.py").write_text("x = 1\n")
    msg = "I edit real.py, e.g. the judge. Check the file.The next step. Create new_gate.py."
    assert babysitter._missing_paths(msg, str(tmp_path)) == ["new_gate.py"]






