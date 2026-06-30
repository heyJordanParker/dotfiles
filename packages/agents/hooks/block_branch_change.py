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

MSG = """BLOCKED: branch changes are BANNED for subagents.

You are a subagent. The worktree is shared with the parent and sibling
subagents. Switching the branch moves HEAD under everyone — silently
corrupting their work. The main session handles branch changes; you do not.

Do NOT run: git switch, git checkout <branch>, git checkout -b/-B,
or git branch -m/-M/-d/-D — and do not route around this via an alias,
sh -c, or git -c alias.*=switch.

If a branch change is genuinely required, return to the user and state
plainly that a branch change is needed. The user runs it."""


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
