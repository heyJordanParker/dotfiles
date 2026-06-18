#!/usr/bin/env python3
"""Archive a subagent's tracer log when that subagent stops.

CLI: archive_subagent_log.py <session_id> <agent_id>
"""

import os
import shutil
import subprocess
import sys


def main(argv):
    session_id = argv[0] if len(argv) > 0 else ""
    agent_id = argv[1] if len(argv) > 1 else ""
    if not session_id or not agent_id:
        return 0
    try:
        r = subprocess.run(
            ["git", "-C", os.getcwd(), "rev-parse", "--show-toplevel"],
            capture_output=True, text=True,
        )
        repo_root = r.stdout.strip() if r.returncode == 0 else ""
    except Exception:
        repo_root = ""
    if not repo_root:
        return 0

    base = os.path.join(repo_root, ".tracer-cache", "sessions", session_id)
    active = os.path.join(base, agent_id)
    archived = os.path.join(base, "archived", agent_id)

    if not os.path.isdir(active):
        return 0
    if os.path.isdir(archived):
        try:
            shutil.rmtree(archived)
        except Exception:
            return 0
    try:
        os.makedirs(os.path.join(base, "archived"), exist_ok=True)
    except Exception:
        return 0
    try:
        shutil.move(active, archived)
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
