#!/usr/bin/env python3
"""SessionStart: inject the `trace context` repo primer as additionalContext."""

import shutil
import subprocess
import sys

from lib import feedback

BINDING = {
    "events": {"SessionStart": ["startup|resume|clear|compact"]},
    "timeout": 20,
    "harness": "all",
}


def main():
    try:
        sys.stdin.read()
    except Exception:
        pass
    if not shutil.which("trace"):
        return 0
    try:
        g = subprocess.run(["git", "rev-parse", "--is-inside-work-tree"], capture_output=True)
        if g.returncode != 0:
            return 0
    except Exception:
        return 0
    try:
        out = subprocess.run(["trace", "context"], capture_output=True, text=True, timeout=12)
    except Exception:
        return 0
    if out.returncode != 0:
        return 0
    output = out.stdout.rstrip("\n")
    if not output:
        return 0
    feedback.context("load_trace_context", "SessionStart", output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
