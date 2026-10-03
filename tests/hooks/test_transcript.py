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


def test_model_reads_the_newest_model_of_either_transcript():
    claude = [
        {"type": "user", "message": {"content": "hi"}},
        {"type": "assistant", "message": {"model": "claude-opus-5-5", "content": []}},
        {"type": "assistant", "message": {"model": "<synthetic>", "content": []}},
    ]
    codex = [
        {"type": "turn_context", "payload": {"model": "gpt-5.5"}},
        {"type": "turn_context", "payload": {"model": "gpt-5.6-luna"}},
        {"type": "response_item", "payload": {"role": "assistant"}},
    ]
    assert transcript.model(claude) == "claude-opus-5-5"
    assert transcript.model(codex) == "gpt-5.6-luna"
    assert transcript.model([{"type": "user", "message": {"content": "hi"}}]) == ""


def test_architect_messages_read_a_message_absorbed_mid_turn():
    def queued(prompt, mode, origin):
        return {"type": "attachment", "attachment": dict(
            {"type": "queued_command", "prompt": prompt, "commandMode": mode},
            **({"origin": {"kind": origin}} if origin else {}))}
    tool_result = {"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "t1"}]}}
    turn = [{"type": "user", "promptSource": "typed", "message": {"content": "start"}},
            {"type": "assistant", "message": {"content": []}},
            tool_result]

    assert transcript.architect_messages(
        turn + [queued("careful with the hooks", "prompt", "human")]) == ["start", "careful with the hooks"]
    assert transcript.architect_messages(
        turn + [queued("<task-notification>done</task-notification>", "task-notification", None)]) == ["start"]
    assert transcript.architect_messages(
        turn + [queued("run the suite", "prompt", "coordinator")]) == ["start"]


def test_architect_messages_read_every_message_after_the_stop_boundary():
    tool_result = {"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "t1"}]}}
    queued = {"type": "attachment", "attachment": {
        "type": "queued_command", "prompt": "message B", "commandMode": "prompt",
        "origin": {"kind": "human"}}}
    recs = [
        {"type": "system", "subtype": "stop_hook_summary"},
        {"type": "user", "promptSource": "typed", "message": {"content": "message A"}},
        {"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Read"}]}},
        tool_result,
        queued,
        {"type": "assistant", "message": {"content": [{"type": "text", "text": "reply"}]}},
    ]

    assert transcript.architect_messages(recs) == ["message A", "message B"]


def test_architect_messages_read_only_the_current_codex_turn():
    recs = [
        {"type": "session_meta", "payload": {"originator": "codex_work_desktop", "source": "desktop"}},
        {"type": "event_msg", "payload": {"type": "task_started"}},
        {"type": "response_item", "payload": {"role": "user", "content": [{"type": "input_text", "text": "first"}]}},
        {"type": "response_item", "payload": {"role": "assistant", "content": []}},
        {"type": "event_msg", "payload": {"type": "task_started"}},
        {"type": "response_item", "payload": {"role": "user", "content": [{"type": "input_text", "text": "second"}]}},
    ]

    assert transcript.architect_messages(recs) == ["second"]


def test_blocks_empty_when_content_is_not_a_list():
    assert transcript.blocks({"message": {"content": "hi"}}) == []
    assert transcript.blocks({"message": None}) == []
    assert transcript.blocks({}) == []
