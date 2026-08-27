"""Behavioral tests for babysitter.py — the one stop gate.

The LLM verdict is stubbed, so these pin the deterministic shell around it: the
agent-session skip, the allow path (exit 0, silent) and the concern path (exit 0,
the model's reason surfaced as a systemMessage on stdout via
feedback.raise_concern), plus the facts that decide which Rules the call carries
at all. The verdict itself is judged by replaying real moments through
tests/hooks/scenarios/.
"""

import os

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


# --- think.md, behind THINK_BEFORE_TALKING ----------------------------------


def _event(tmp_path):
    return {"session_id": "bs1", "cwd": str(tmp_path)}


def _with_draft(tmp_path, monkeypatch, text, written_at=1000, turn_start=1000,
                slug="007-the-work"):
    """think.md in an Evidence directory, with a chosen write time against a chosen
    turn boundary.

    Writes a real file under a real docs/agents tree so the reader's own glob and
    window check run — those are the contract, and stubbing the read would test
    neither."""
    from lib import session_state
    run = tmp_path / "docs" / "agents" / slug
    run.mkdir(parents=True)
    draft = run / "think.md"
    draft.write_text(text)
    os.utime(draft, (written_at, written_at))
    monkeypatch.setattr(babysitter, "load_state",
                        lambda sid: {"current_turn_start": turn_start})
    monkeypatch.setattr(session_state, "_now", lambda: written_at + 1)
    return draft


def test_think_md_stays_out_while_the_flag_is_unset(tmp_path, monkeypatch):
    """The whole point of the flag: with it unset the prompt is what it was before
    think.md existed, so a live session cannot change because a file appeared on
    disk."""
    _with_draft(tmp_path, monkeypatch, "settled: rename the column")
    monkeypatch.delenv("THINK_BEFORE_TALKING", raising=False)
    assert babysitter._draft_facts(_event(tmp_path)) == ""

    off = babysitter._eval_prompt("ask", "reply", "action", "execute", "build", True)
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    on = babysitter._eval_prompt("ask", "reply", "action", "execute", "build", True,
                                 draft_facts=babysitter._draft_facts(_event(tmp_path)))
    assert "### Name a Decision think.md settled" not in off
    assert "### Name a Decision think.md settled" in on
    assert "rename the column" in on


def test_a_think_md_from_an_earlier_turn_is_ignored(tmp_path, monkeypatch):
    """An Evidence directory outlives its run, so without the turn check every
    later turn is judged against a finished one — a false concern on every reply
    where the agent answered without rewriting the file."""
    _with_draft(tmp_path, monkeypatch, "settled last turn", written_at=1000,
                turn_start=2000)
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert babysitter._draft_facts(_event(tmp_path)) == ""


def test_think_md_is_found_under_a_slug_the_hook_never_knew(tmp_path, monkeypatch):
    """The agent names its own Evidence directory, so the reader finds the file by
    its fixed name and write time. Rebuilding the path on this side would need a
    slug only the agent chose."""
    _with_draft(tmp_path, monkeypatch, "this run's findings", slug="042-whatever")
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert "this run's findings" in babysitter._draft_facts(_event(tmp_path))


def test_the_newest_think_md_in_the_window_wins(tmp_path, monkeypatch):
    """A session that worked two runs leaves two files. The one written last is
    this reply's, and reading the other would judge the reply against work it was
    not about."""
    _with_draft(tmp_path, monkeypatch, "the older run", written_at=1000,
                slug="001-older")
    _with_draft(tmp_path, monkeypatch, "the newer run", written_at=1400,
                slug="002-newer")
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert "the newer run" in babysitter._draft_facts(_event(tmp_path))


def test_a_think_md_stamped_in_the_future_is_ignored(tmp_path, monkeypatch):
    """A lower bound alone lets a file stamped ahead of the clock read as fresh on
    every turn for good, so the window is closed at both ends."""
    from lib import session_state
    _with_draft(tmp_path, monkeypatch, "settled: rename the column",
                written_at=9000, turn_start=1000)
    monkeypatch.setattr(session_state, "_now", lambda: 2000)
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert babysitter._draft_facts(_event(tmp_path)) == ""


def test_a_session_with_no_think_md_is_silent(tmp_path, monkeypatch):
    """An agent that never wrote one degrades to today's judgement rather than a
    block: the Hook fails open on every input it does not have."""
    monkeypatch.setattr(babysitter, "load_state",
                        lambda sid: {"current_turn_start": 1000})
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert babysitter._draft_facts(_event(tmp_path)) == ""


def test_an_empty_think_md_is_silent(tmp_path, monkeypatch):
    _with_draft(tmp_path, monkeypatch, "   \n")
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert babysitter._draft_facts(_event(tmp_path)) == ""


def test_think_md_survives_a_wake_up_inside_the_same_turn(tmp_path, monkeypatch):
    """current_turn_start advances on a human prompt only, so a task notification
    resuming the same turn keeps that boundary. A file written earlier in the turn
    is this turn's, and dropping it there would blind the judge on exactly the
    replies that follow dispatched work."""
    _with_draft(tmp_path, monkeypatch, "settled: the gate reads the event",
                written_at=1500, turn_start=1000)
    monkeypatch.setenv("THINK_BEFORE_TALKING", "1")
    assert "the gate reads the event" in babysitter._draft_facts(_event(tmp_path))


def test_missing_paths_names_only_what_does_not_resolve(tmp_path):
    """A file the message proposes creating can never be in the opened set, so
    without this the unopened-edit Rule reads a new file as code changed blind."""
    (tmp_path / "real.py").write_text("x = 1\n")
    msg = "I edit real.py, e.g. the judge. Check the file.The next step. Create new_gate.py."
    assert babysitter._missing_paths(msg, str(tmp_path)) == ["new_gate.py"]






