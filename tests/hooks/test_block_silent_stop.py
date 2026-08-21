import json
import os
import subprocess

from conftest import PY_HOOKS


def _run(transcript_path):
    return subprocess.run(
        ["python3", os.path.join(PY_HOOKS, "block_silent_stop.py")],
        input=json.dumps({"transcript_path": transcript_path}),
        text=True,
        capture_output=True,
    )


def test_blocks_turn_that_ran_tools_and_wrote_no_text(write_transcript):
    path = write_transcript([
        {"type": "user", "message": {"role": "user", "content": "status?"}},
        {"type": "assistant", "message": {"role": "assistant", "stop_reason": "tool_use", "content": [
            {"type": "tool_use", "id": "t1", "name": "ScheduleWakeup",
             "input": {"delaySeconds": 60}},
        ]}},
    ])

    result = _run(path)

    assert result.returncode == 2


def test_allows_reply_whose_text_record_has_not_flushed(write_transcript):
    # Observed live 2026-08-21: the reply flushes as an empty-thinking record
    # with stop_reason end_turn, then the text record seconds later. The hook
    # fired between the two and blocked a turn that ended in a full reply.
    path = write_transcript([
        {"type": "user", "message": {"role": "user", "content": "status?"}},
        {"type": "assistant", "message": {"role": "assistant", "stop_reason": "tool_use", "content": [
            {"type": "tool_use", "id": "t1", "name": "Bash",
             "input": {"command": "true"}},
        ]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]}},
        {"type": "assistant", "message": {"role": "assistant", "stop_reason": "end_turn", "content": [
            {"type": "thinking", "thinking": "", "signature": "x"},
        ]}},
    ])

    result = _run(path)

    assert result.returncode == 0


def test_allows_turn_that_replied_in_text(write_transcript):
    path = write_transcript([
        {"type": "user", "message": {"role": "user", "content": "status?"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "t1", "name": "Bash",
             "input": {"command": "true"}},
        ]}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "One agent is live; the gate is green."},
        ]}},
    ])

    result = _run(path)

    assert result.returncode == 0
