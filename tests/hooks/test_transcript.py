"""Unit tests for the shared transcript layer (lib/transcript.py).

Everything in the hook suite that reads Claude's JSONL goes through this module,
so its record parsing, turn boundary, and block filtering are the
highest-leverage things to pin down.
"""

from lib import transcript


def test_records_parses_and_skips_bad_lines(tmp_path):
    p = tmp_path / "t.jsonl"
    p.write_text('{"type":"user"}\nnot json\n\n{"type":"assistant"}\n')
    recs = transcript.records(str(p))
    assert [r["type"] for r in recs] == ["user", "assistant"]


def test_records_start_after_the_newest_compaction(tmp_path):
    p = tmp_path / "t.jsonl"
    p.write_text(
        '{"type":"user","message":{"content":"first window"}}\n'
        '{"type":"user","message":{"content":"This session is being continued from a"}}\n'
        '{"type":"user","message":{"content":"second window"}}\n'
        '{"type":"user","message":{"content":"This session is being continued from b"}}\n'
        '{"type":"user","message":{"content":"live"}}\n'
        '{"type":"assistant","message":{"content":"answer"}}\n')
    assert [transcript.text_of(r) for r in transcript.records(str(p))] == ["live", "answer"]


def test_records_keep_lines_whole_across_chunks(tmp_path, monkeypatch):
    monkeypatch.setattr(transcript, "_CHUNK", 5)
    p = tmp_path / "t.jsonl"
    p.write_text('{"type":"user","message":{"content":"héllo wörld"}}\n'
                 '{"type":"assistant","message":{"content":"ünïcode"}}', encoding="utf-8")
    assert [transcript.text_of(r) for r in transcript.records(str(p))] == ["héllo wörld", "ünïcode"]


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
