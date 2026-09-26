#!/usr/bin/env python3
"""Block git operations that restore files from git or throw away uncommitted work.

Commands are read through lib.command's quote-aware parser, so a command that only
mentions one of these in quoted text (a search pattern, a commit message) runs, and
a command carried inside `bash -c` or an ssh line is judged like one typed directly.

block-git-revert.sh is the plugin-distributed shell copy of this source.
"""

import os
import re
import sys

from lib import feedback
from lib.command import all_segments, git_subcommand, is_redirect
from lib.event import command_str, field, read_event

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

FORCE_MSG = """BLOCKED: a forced checkout throws away every uncommitted edit in the tree.

Other agents' uncommitted work lives in this worktree. Run the checkout without
--force. Git then refuses it when it would overwrite an edit, and the main
session decides what happens to that edit."""

RESTORE_MSG = """BLOCKED: git restore is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually."""

CLEAN_MSG = """BLOCKED: agents never run git clean.

It deletes every untracked file in the tree, other agents' new files included,
and nothing brings them back. Remove a file you created with `rm <path>`."""

STASH_MSG = """BLOCKED: agents never run git stash.

Other agents' uncommitted work lives in this worktree, and a stash hides it.
Only `git stash list` and `git stash show` run.

If you are proving a failure is pre-existing: stop, and report the exact
command and its red output."""

_FORCE_FLAGS = frozenset(("-f", "--force", "--discard-changes"))
_BRANCH_FLAGS = frozenset(("-b", "-B", "--orphan"))
_UNPARSEABLE = re.compile(r"git\s+(reset|restore|clean|stash|checkout)")
_UNPARSEABLE_MSG = {
    "reset": RESET_MSG,
    "restore": RESTORE_MSG,
    "clean": CLEAN_MSG,
    "stash": STASH_MSG,
    "checkout": CHECKOUT_FILES_MSG,
}


def block(msg):
    return feedback.block("block_git_revert", msg)


def _names_path(arg, cwd):
    return ("." in arg or "/" in arg or any(c in arg for c in "*?[")
            or os.path.exists(os.path.join(cwd, arg)))


def _without_redirects(args):
    """args without shell redirects: the operator, its target, and the descriptor before it."""
    out, skip = [], False
    for arg in args:
        if skip:
            skip = False
        elif is_redirect(arg):
            if out and out[-1].isdigit():
                out.pop()
            skip = True
        else:
            out.append(arg)
    return out


def _checkout_refusal(args, cwd):
    """Why this checkout overwrites files, or "" when it only moves between branches.

    --ours/--theirs resolve a merge or rebase conflict and stay allowed.
    """
    if "--ours" in args or "--theirs" in args:
        return ""
    if _FORCE_FLAGS & set(args):
        return FORCE_MSG
    if "--" in args:
        before = [a for a in args[:args.index("--")] if not a.startswith("-")]
        return CHECKOUT_REF_MSG if before else CHECKOUT_FILES_MSG
    if any(a.startswith("--pathspec-from-file") for a in args):
        return CHECKOUT_FILES_MSG
    if _BRANCH_FLAGS & set(args):
        return ""
    positional = [a for a in args if not a.startswith("-")]
    if len(positional) > 1:
        return CHECKOUT_REF_MSG
    if positional and _names_path(positional[0], cwd):
        return CHECKOUT_FILES_MSG
    return ""


def _refusal(words, cwd):
    subcommand = git_subcommand(words)
    if not subcommand:
        return ""
    args = words[words.index(subcommand) + 1:]
    if subcommand == "reset":
        return RESET_MSG
    if subcommand == "restore":
        return RESTORE_MSG
    if subcommand == "clean":
        return CLEAN_MSG
    if subcommand == "stash":
        return "" if args[:1] in (["list"], ["show"]) else STASH_MSG
    if subcommand == "checkout":
        return _checkout_refusal(_without_redirects(args), cwd)
    if subcommand == "switch" and _FORCE_FLAGS & set(args):
        return FORCE_MSG
    return ""


def main():
    event = read_event()
    command = command_str(event)
    cwd = field(event, "cwd", "") or os.getcwd()

    segs = all_segments(command)
    if segs is None:
        match = _UNPARSEABLE.search(command)
        return block(_UNPARSEABLE_MSG[match.group(1)]) if match else 0

    for words in segs:
        refusal = _refusal(words, cwd)
        if refusal:
            return block(refusal)
    return 0


if __name__ == "__main__":
    sys.exit(main())
