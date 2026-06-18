#!/usr/bin/env python3
"""SessionStart: re-run `trace context prime` so the tracer log mirrors loaded docs."""

import os
import shutil
import subprocess
import sys

from lib.event import field, read_event

BINDING = {
    "events": {"SessionStart": ["startup|resume|clear|compact"]},
    "timeout": 20,
    "harness": "all",
}


def main():
    event = read_event()
    if not shutil.which("trace"):
        return 0
    session_id = field(event, "session_id", "")
    if not session_id:
        return 0
    # Hand trace the run's own session via AGENT_SESSION_ID — the harness-neutral
    # carrier trace resolves first — on a local copy only, never mutating
    # os.environ; CLAUDE_CODE_SESSION_ID stays as the launcher set it so
    # owner_session can resolve the governing mode on a nested codex run.
    env = dict(os.environ)
    env["AGENT_SESSION_ID"] = session_id
    agent_id = field(event, "agent_id", "")
    if agent_id:
        env["TRACER_AGENT_ID"] = agent_id
    source = field(event, "source", "")
    # `clear` and `compact` drop previously-surfaced docs from context while the
    # harness re-injects only the global + project-root chain. Reset the view first
    # so nested Claude.md files (surfaced by enrich-on-read) re-inject on next read
    # instead of being skipped as already-loaded; prime then re-records the live
    # chain. Same env so the session id propagates and the reset isn't a silent no-op.
    if source in ("clear", "compact"):
        try:
            subprocess.run(
                ["trace", "docs", "reset"],
                capture_output=True, timeout=10, env=env,
            )
        except Exception:
            pass
    reason = "post_compact" if source == "compact" else "session_start"
    try:
        subprocess.run(
            ["trace", "context", "prime", "--reason", reason],
            capture_output=True, timeout=12, env=env,
        )
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
