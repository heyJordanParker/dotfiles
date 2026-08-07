#!/usr/bin/env python3
"""Wake an idle Claude session once when one of its codex jobs has failed."""

import time

from lib import codex_run, feedback
from lib.event import field, read_event
from lib.session_state import load_state

BINDING = {
    "events": {"Stop": [], "SubagentStop": []},
    "harness": "claude",
    "asyncRewake": True,
}


def main():
    session_id = field(read_event(), "session_id", "")
    if not session_id:
        return 0
    directory = codex_run.session_state._session_dir(session_id)
    if not directory:
        return 0
    turn_start = load_state(session_id).get("current_turn_start")
    for record in codex_run._records_in(directory):
        if (record.get("status") != "failed" or record.get("rewoken_at")
                or record.get("ended_at") is None or turn_start is None
                or record["ended_at"] < turn_start):
            continue
        record["rewoken_at"] = int(time.time())
        codex_run._save_record(record)
        return feedback.block(
            "rewake_codex_failure",
            "codex job %s failed: %s" % (record.get("job"), record.get("error") or "no answer"),
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
