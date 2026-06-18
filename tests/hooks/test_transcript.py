"""Unit tests for the shared transcript layer (lib/transcript.py).

Everything in the hook suite that reads Claude's JSONL goes through this module,
so its boundary detection, evidence formatting, and tool-outcome pairing are the
highest-leverage things to pin down.
"""

from lib import transcript


def test_records_parses_and_skips_bad_lines(tmp_path):
    p = tmp_path / "t.jsonl"
    p.write_text('{"type":"user"}\nnot json\n\n{"type":"assistant"}\n')
    recs = transcript.records(str(p))
    assert [r["type"] for r in recs] == ["user", "assistant"]


def test_records_missing_path_is_empty():
    assert transcript.records("") == []
    assert transcript.records("/no/such/file.jsonl") == []


def test_is_real_user_distinguishes_tool_results():
    assert transcript.is_real_user({"type": "user", "message": {"content": "hi"}})
    tool_result = {"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "t1"}]}}
    assert not transcript.is_real_user(tool_result)
    assert not transcript.is_real_user(
        {"type": "assistant", "message": {"content": []}})


def test_current_turn_is_records_after_last_real_user():
    recs = [
        {"type": "user", "message": {"content": "first"}},
        {"type": "assistant", "message": {"content": [{"type": "text", "text": "a"}]}},
        {"type": "user", "message": {"content": "second"}},   # the boundary
        {"type": "assistant", "message": {"content": [{"type": "text", "text": "b"}]}},
    ]
    turn = transcript.current_turn(recs)
    assert len(turn) == 1
    assert turn[0]["message"]["content"][0]["text"] == "b"


def test_current_turn_lines_matches_boundary(write_transcript):
    path = write_transcript([
        {"type": "user", "message": {"content": "go"}},
        {"type": "assistant", "message": {"content": [{"type": "text", "text": "x"}]}},
    ])
    lines = transcript.current_turn_lines(path)
    assert any('"text":"x"' in ln for ln in lines)   # assistant response in the turn
    assert not any("go" in ln for ln in lines)       # boundary user line excluded


def test_turn_evidence_keeps_responses_marks_thinking_and_edit_outcomes():
    turn = [
        {"type": "assistant", "message": {"content": [
            {"type": "thinking", "thinking": "z" * 40},
            {"type": "text", "text": "FULL DELIVERABLE here"}]}},
        {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "t1", "name": "Edit",
             "input": {"file_path": "/r/x.py"}},
            {"type": "tool_use", "id": "t2", "name": "Edit",
             "input": {"file_path": "/r/y.py"}}]}},
        {"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": "t1"},
            {"type": "tool_result", "tool_use_id": "t2",
             "is_error": True, "content": "boom"}]}},
        {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "done, see above"}]}},
    ]
    ev = transcript.turn_evidence(turn)
    assert "FULL DELIVERABLE here" in ev               # response kept whole
    assert "[thinking: 40 chars]" in ev                # thinking compressed, raw gone
    assert "z" * 40 not in ev
    assert "/r/x.py -> ok" in ev                        # successful edit
    assert "/r/y.py -> FAILED" in ev                    # failed edit reflected
    assert "Edits this turn: 1 finished, 1 failed." in ev


def test_tool_outcomes_pairs_by_id():
    recs = [
        {"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": "ok1"},
            {"type": "tool_result", "tool_use_id": "bad1", "is_error": True}]}},
    ]
    out = transcript.tool_outcomes(recs)
    assert out == {"ok1": False, "bad1": True}


def test_plan_content_returns_last_exitplanmode_plan_whole():
    recs = [
        {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "ExitPlanMode",
             "input": {"plan": "PLAN ONE"}}]}},
        {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "ExitPlanMode",
             "input": {"plan": "PLAN\nTWO\nlines"}}]}},
    ]
    assert transcript.plan_content(recs) == "PLAN\nTWO\nlines"


def test_recent_user_texts_last_n_messages_whole():
    recs = [{"type": "user", "message": {"content": f"msg {i}"}} for i in range(6)]
    out = transcript.recent_user_texts(recs, 2)
    assert out == "msg 4\nmsg 5"


def test_conversation_stream_caps_and_skips_noise():
    noise = "<task-notification>x</task-notification>"  # XML-noise user line is skipped
    recs = [
        {"type": "user", "message": {"content": noise}},
        {"type": "user", "message": {"content": "u" * 500}},
        {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "a" * 500}]}},
    ]
    lines = transcript.conversation_stream(recs, user_cap=200, assistant_cap=300)
    assert lines[0] == "U|" + "u" * 200
    assert lines[1] == "A|" + "a" * 300
    assert len(lines) == 2  # the XML-noise user line is dropped


def test_clamp_bounds_only_when_over():
    assert transcript.clamp("short", 200) == "short"
    assert transcript.clamp("x" * 300, 200) == "x" * 200


# ---------------------------------------------------------------------------
# Extension coverage: block filtering, the size threshold, the Bash evidence
# target, the on-disk plan fallback, and the list/tool-only content shapes that
# the per-line views must handle. None of these had a shell predecessor.
# ---------------------------------------------------------------------------

def _tool_only_user():
    """A user record carrying only a tool_result — the views must skip it."""
    return {"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "t1"}]}}


def test_blocks_filters_by_kind_and_drops_non_dicts():
    rec = {"message": {"content": [
        {"type": "text", "text": "a"},
        {"type": "tool_use", "name": "Edit"},
        "a bare string that is not a block",
    ]}}
    assert [b["type"] for b in transcript.blocks(rec)] == ["text", "tool_use"]
    assert [b["text"] for b in transcript.blocks(rec, "text")] == ["a"]


def test_blocks_empty_when_content_is_not_a_list():
    assert transcript.blocks({"message": {"content": "hi"}}) == []
    assert transcript.blocks({"message": None}) == []
    assert transcript.blocks({}) == []


def test_assistant_text_len_counts_text_plus_one_per_block():
    recs = [
        {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "abc"},          # 3 + 1
            {"type": "thinking", "thinking": "ignored"}]}},
        # second assistant text block: 2 + 1
        {"type": "assistant", "message": {"content": [{"type": "text", "text": "de"}]}},
        {"type": "user", "message": {"content": "not counted"}},
    ]
    assert transcript.assistant_text_len(recs) == 3 + 1 + 2 + 1


def test_turn_evidence_truncates_long_bash_command():
    turn = [{"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": "b1", "name": "Bash",
         "input": {"command": "x" * 200}}]}}]
    ev = transcript.turn_evidence(turn)
    assert "x" * 120 + "…" in ev   # first 120 chars then an ellipsis
    assert "x" * 200 not in ev


def test_turn_evidence_empty_when_no_assistant_records():
    turn = [{"type": "user", "message": {"content": "just a user line"}}]
    assert transcript.turn_evidence(turn) == ""


def test_plan_content_falls_back_to_slug_file(tmp_path, monkeypatch):
    # No ExitPlanMode plan in the transcript, but a slug points at a plan file.
    home = tmp_path
    plans = home / ".claude" / "plans"
    plans.mkdir(parents=True)
    (plans / "my-feature.md").write_text("PLAN FROM DISK")
    monkeypatch.setattr("os.path.expanduser", lambda p: p.replace("~", str(home)))
    recs = [{"type": "assistant", "slug": "my-feature", "message": {"content": []}}]
    assert transcript.plan_content(recs) == "PLAN FROM DISK"


def test_plan_content_empty_when_no_plan_and_no_slug():
    recs = [{"type": "assistant", "message": {"content": []}}]
    assert transcript.plan_content(recs) == ""


def test_recent_user_texts_joins_list_blocks_and_skips_tool_only():
    recs = [
        {"type": "user", "message": {"content": [
            {"type": "text", "text": "hello"}, {"type": "text", "text": "there"}]}},
        _tool_only_user(),   # contributes nothing
        {"type": "user", "message": {"content": "plain string"}},
    ]
    assert transcript.recent_user_texts(recs, 4) == "hello there\nplain string"


def test_conversation_stream_skips_non_string_user_content():
    recs = [
        _tool_only_user(),   # list-content user record is skipped (only scalar streams)
        {"type": "user", "message": {"content": "real question"}},
        {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "answer"}]}},
    ]
    lines = transcript.conversation_stream(recs)
    assert lines == ["U|real question", "A|answer"]
