#!/usr/bin/env python3
"""Block file mutations while session state is "proposing".

Mutation targets are found by quote-aware tokenizing (see command.segments), so a
`>` only counts as a redirect when it's a real shell operator — never when it sits
inside a quoted argument. A read-only command like `gh api -f q='a > b'` therefore
isn't mistaken for a redirect into a repo file and false-blocked.

A line whose real command is not in the line — inline runtime code, a script file
that is not our own tooling, an unbalanced quote — comes back from the shared reader
as None and is refused. It used to run: the interpreter check this hook owned
privately was spelled `if proposing and …`, so a mode-banned orchestrator walked
straight past it and wrote through `python3 -c`.
"""

import os
import sys

from lib import feedback
from lib.command import all_segments, git_subcommand, in_tmp, is_ours, mutation_targets
from lib.event import canonical_tool, field, patch_target, read_event
from lib.session_mode import permits, state

BINDING = {
    "events": {
        "PreToolUse": ["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"],
    },
    "harness": "all",
    "timeout": 5,
}

BLOCK_MSG = """BLOCKED: this session is proposing, so code edits wait.

The Architect moves it to executing by typing /execute, /orchestrate, or /build.
Plans, shaping docs, and Evidence stay writable in docs/plans/, docs/shaping/,
docs/agents/, and /tmp/. Never write code in those directories to avoid this block."""

MODE_MSG = """BLOCKED: this Agent's mode does not write files.

In orchestrate mode, dispatch a subagent to make the edit. In interview mode,
report the edit the work needs."""

_DEVICES = {"/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty"}

# git subcommands that mutate the working tree and are not already blocked
# unconditionally by block_git_revert (reset/checkout/restore/stash/clean).
# `git rm --cached` changes staging alone, like `git add`, and stays allowed.
_GIT_TREE_MUTATORS = ("rm", "mv")


def _git_tree_mutation(words):
    subcommand = git_subcommand(words)
    return subcommand in _GIT_TREE_MUTATORS and "--cached" not in words


def _allowed_target(t, cwd):
    """True = allowed, False = blocked (a mutation that lands in our tree)."""
    t = t.strip().strip('"').strip("'")
    if t in _DEVICES:
        return True
    if t.startswith("~"):
        t = os.path.expanduser("~") + t[1:]
    if not t.startswith("/"):
        t = os.path.join(cwd, t)
    t = os.path.normpath(t)
    # t + "/" so the directory itself is allowed, not only paths inside it —
    # `mkdir -p docs/agents` must pass, not just writes to files under it.
    probe = t + "/"
    for d in ("/docs/shaping/", "/docs/plans/", "/docs/agents/",
              "/.claude/shaping/", "/.claude/plans/"):
        if d in probe:
            return True
    if in_tmp(t):
        return True
    if t == cwd or t.startswith(cwd + "/") or is_ours(t):
        return False
    return True


UNREADABLE_MSG = """BLOCKED: this command line cannot be read by the write gate.

Inline code (`python3 -c`), a script that is not our own tooling, or an unbalanced
quote hides what actually runs, so the whole line is refused rather than guessed at.

Run the operation as a plain command."""

SCRIPT_WARNING = ("Heads up — a proposal is expected: you can write this script, but "
                  "executing it (python/node/bash/…) is blocked until the proposal is approved.")

_SCRIPT_EXTS = (".py", ".sh", ".bash", ".zsh", ".js", ".mjs", ".cjs", ".ts", ".rb", ".pl")


def _is_script(path):
    return path.strip().strip("\"'").endswith(_SCRIPT_EXTS)


def warn(msg):
    feedback.context("block_writes", "PreToolUse", msg)


def block(msg=BLOCK_MSG):
    return feedback.block("block_writes", msg)


def main():
    event = read_event()
    # A dispatched orchestrator is forbidden MUTATIONS, not Bash: the same target
    # parser the proposing arm uses decides, so `pytest`, `ls`, `echo`, and
    # `git status` all run — an orchestrator has to be able to validate what its
    # subagents built — and only a command that changes the tree is refused.
    banned = not permits(event, "write")
    # The proposing arm governs every event, a dispatched agent's included: it
    # answers to the stage of the session that launched it, so a codex run under a
    # proposing launcher writes nothing in the tree either.
    proposing = state(event) == "propose"
    if not proposing and not banned:
        return 0
    refusal = MODE_MSG if banned else BLOCK_MSG

    cwd = field(event, "cwd", "") or os.getcwd()
    file_path = patch_target(event)
    command = field(event, "tool_input.command", "")

    if canonical_tool(event) == "write":
        if banned:
            return block(MODE_MSG)
        # codex delivers an apply_patch as tool_input.command carrying the patch body,
        # so the shell parse below would read the diff's own text as commands — a `+`
        # line adding a redirect reads as a redirect. `patch_target` names the file
        # the patch touches, so a codex write is judged against its target like any
        # other; a patch naming no target leaves nothing to judge and is refused.
        if not file_path:
            return block()

    if file_path:
        if not _allowed_target(file_path, cwd):
            return block(refusal)
        if _is_script(file_path):
            warn(SCRIPT_WARNING)
        return 0

    if not command:
        return 0

    segs = all_segments(command)
    if segs is None:
        return block(UNREADABLE_MSG)
    if any(_git_tree_mutation(seg) for seg in segs):
        return block(refusal)
    for seg in segs:
        for target in mutation_targets(seg):
            if not _allowed_target(target, cwd):
                return block(refusal)
    return 0


if __name__ == "__main__":
    sys.exit(main())
