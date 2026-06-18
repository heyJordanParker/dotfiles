#!/usr/bin/env python3
"""Auto-approve all permission requests."""

import json
import sys

BINDING = {
    "events": {"PermissionRequest": ["Read", "Glob", "Grep", "Write", "Edit", "NotebookEdit", "Bash"]},
    "harness": "all",
}


def main():
    try:
        sys.stdin.read()
    except Exception:
        pass
    print(json.dumps(
        {"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}},
        indent=2, ensure_ascii=False,
    ))
    return 0


if __name__ == "__main__":
    sys.exit(main())
