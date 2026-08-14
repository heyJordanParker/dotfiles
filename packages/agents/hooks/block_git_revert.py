#!/usr/bin/env python3
"""Block destructive git operations that agents misuse for "reverting".

block-git-revert.sh is the plugin-distributed shell copy of this source.
"""

import re
import sys

from lib import feedback
from lib.command import git_normalize
from lib.event import command_str, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

RESET_MSG = """BLOCKED: git reset is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually."""

CHECKOUT_REF_MSG = """BLOCKED: git checkout <ref> -- <path> is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually."""

CHECKOUT_FILES_MSG = """BLOCKED: git checkout of files is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually."""

RESTORE_MSG = """BLOCKED: git restore is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually."""

STASH_MSG = """BLOCKED: git stash is BANNED for agents. This is not a soft limit. Never run it.

git stash hides or discards uncommitted work in a worktree shared by other
agents. It is the single most common way agent work is silently lost. It has
already destroyed real work in this repo.

Do NOT stash. Do NOT pop, drop, clear, push, apply, or save a stash. Do NOT
hide a mutating stash behind a trailing `&& git stash list`, an alias, sh -c,
or git -c alias.*=stash. Adding a read-only stash command does not make this
allowed.

To run something against a clean tree: commit your work first, then run it.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually."""


def block(msg):
    return feedback.block("block_git_revert", msg)


def main():
    event = read_event()
    command = command_str(event)

    normalized = git_normalize(command)

    # Pattern 1: git reset (all forms are destructive)
    if re.search(r"git\s+reset", normalized):
        return block(RESET_MSG)

    # Pattern 2: git checkout of files (not branch switches)
    # Allow: --ours/--theirs (legitimate during merge/rebase conflicts)
    if re.search(r"git\s+checkout\s+.*(--ours|--theirs)", normalized):
        return 0
    # Block: checkout <ref> -- <path>
    if re.search(r"git\s+checkout\s+\S+\s+--\s+", normalized):
        return block(CHECKOUT_REF_MSG)
    # Block: checkout -- <file>, checkout <file.ext>, checkout <path/file>
    if re.search(r"git\s+checkout\s+(--\s+|[^-]\S*\.\S+|[^-]\S*/\S+)", normalized):
        return block(CHECKOUT_FILES_MSG)

    # Pattern 3: git restore
    if re.search(r"git\s+restore", normalized):
        return block(RESTORE_MSG)

    # Pattern 4: git stash is BANNED except read-only list/show
    residual = re.sub(r"git\s+stash\s+(list|show)[^&|;]*", "", normalized)
    if re.search(r"git\s+stash", residual):
        return block(STASH_MSG)

    return 0


if __name__ == "__main__":
    sys.exit(main())
