#!/usr/bin/env python3
"""Terminate codex jobs owned by the Claude session that is ending."""

from lib import codex_run
from lib.event import field, read_event

BINDING = {
    "events": {"SessionEnd": []},
    "harness": "claude",
    "timeout": 1,
}


def main():
    session_id = field(read_event(), "session_id", "")
    for record in codex_run.live_jobs(session_id):
        codex_run.end_job(record)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
