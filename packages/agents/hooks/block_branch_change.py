#!/usr/bin/env python3
"""Block branch-changing git operations for subagents only.

Subagent-only is enforced by the agent_id gate below (and by the hook's
declared binding), not by directory.
"""

import re
import sys

from lib import feedback
from lib.command import git_normalize
from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

MSG = """BLOCKED: subagents do not change branches.

The worktree is shared, so a branch change moves HEAD under the main session and every sibling.
If the work needs a branch change, say so in your report. The main session makes it."""


def main():
    event = read_event()
    agent_id = field(event, "agent_id", "")
    if not agent_id:
        return 0  # main session — allow

    command = command_str(event)
    normalized = git_normalize(command)

    # ` -- ` separator is file-revert syntax, owned by block-git-revert.
    if re.search(r"\s--\s", normalized):
        return 0

    if re.search(
        r"(git\s+switch(\s|$))"
        r"|(git\s+branch\s+-[mMdD])"
        r"|(git\s+checkout\s+-[bB])"
        r"|(git\s+checkout\s+[A-Za-z0-9_@][A-Za-z0-9_@-]*(\s|$))",
        normalized,
    ):
        return feedback.block("block_branch_change", MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
