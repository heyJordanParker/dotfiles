#!/usr/bin/env python3
"""SessionStart: inject the `trace context` repo primer as additionalContext."""

import subprocess
import sys

from lib import feedback, tracer
from lib.event import field, read_event

BINDING = {
    "events": {"SessionStart": ["startup|resume|clear|compact"]},
    "timeout": 20,
    "harness": "all",
    "standalone": True,
    "additionalContextLimit": 0,
}


def main():
    if not tracer.available():
        return 0
    event = read_event()
    cwd = field(event, "cwd", "") or None
    if subprocess.run(["git", "rev-parse", "--is-inside-work-tree"], capture_output=True, cwd=cwd).returncode != 0:
        return 0
    code, output, _ = tracer.run(event, "context", timeout=12)
    output = output.rstrip("\n")
    if code != 0 or not output:
        return 0
    feedback.context("load_trace_context", "SessionStart", output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
