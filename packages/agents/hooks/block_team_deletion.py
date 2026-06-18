#!/usr/bin/env python3
"""Block the TeamDelete tool."""

import sys

from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["TeamDelete"]},
    "timeout": 5,
    "harness": "claude",
}

MSG = "BLOCKED: Teams are managed by Jordan. Use SendMessage to reassign teammates."


def main():
    event = read_event()
    if field(event, "tool_name", "") == "TeamDelete":
        sys.stderr.write(MSG + "\n")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
