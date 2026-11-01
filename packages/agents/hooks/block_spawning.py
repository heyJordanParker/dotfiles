#!/usr/bin/env python3
import os
import re
import sys

from classify_intent import blank_spans, forced_commands
from lib import command, feedback
from lib.event import canonical_tool, command_str, field, read_event
from lib.session_mode import is_dispatched, may_spawn, resolve

BINDING = {
    "events": {"PreToolUse": ["Bash", "Agent", "Workflow", "RemoteTrigger"]},
    "harness": "all",
    "timeout": 5,
}
# `codex` and `claude` start a session with anything but a lone version or help flag,
# or a subcommand that only manages the Harness.
_INFO_FLAGS = frozenset(("--version", "-V", "-v", "--help", "-h"))
_MANAGEMENT = {
    "claude": frozenset(("mcp", "plugin", "plugins", "auth", "doctor", "install", "update",
                         "upgrade", "setup-token", "auto-mode", "import", "purge", "logs",
                         "stop", "kill", "rm")),
    "codex": frozenset(("mcp", "plugin", "login", "logout", "completion", "update", "doctor",
                        "features", "help", "apply", "archive", "unarchive", "delete")),
}

MSG = "BLOCKED: %s mode does not spawn subagents.\n\nIn build mode, do the work yourself."

SUBAGENT_MSG = ("BLOCKED: a Subagent never starts another agent.\n\n"
                "Do the work yourself, or return what needs another agent to your dispatcher.")

FABLE_MSG = ("BLOCKED: this launch runs an agent on Fable, and only the architect uses Fable.\n\n"
             "Drop the model override, or name another model.")

ORCHESTRATE_MSG = ("BLOCKED: this launch types /orchestrate into a new session, and only the "
                   "architect makes a session orchestrate.\n\nLaunch it without /orchestrate.")

_SCRIPT_MODEL = re.compile(r"\bmodel\s*:\s*['\"`]([^'\"`]*)", re.I)


def _is_fable(model):
    return "fable" in str(model).lower()


def _claude_launches(event):
    found = command.invocations(command_str(event)) or ()
    return [args for head, args in found if head == "claude"]


def _claude_models(args):
    for i, arg in enumerate(args):
        if arg == "--model" and i + 1 < len(args):
            yield args[i + 1]
        elif arg.startswith("--model="):
            yield arg.split("=", 1)[1]


def _workflow_script(event):
    path = field(event, "tool_input.scriptPath", "")
    if not path:
        return field(event, "tool_input.script", "")
    with open(os.path.join(field(event, "cwd", ""), os.path.expanduser(path)), encoding="utf-8") as fh:
        return fh.read()


def _chooses_fable(event):
    if field(event, "tool_name", "") == "Workflow":
        return any(_is_fable(m) for m in _SCRIPT_MODEL.findall(_workflow_script(event)))
    if canonical_tool(event) == "agent":
        return _is_fable(field(event, "tool_input.model", ""))
    return any(_is_fable(m) for args in _claude_launches(event) for m in _claude_models(args))


def _types_orchestrate(event):
    return any(forced_commands(blank_spans(arg))[1] == "orchestrate"
               for args in _claude_launches(event) for arg in args)


def _spawns(head, args):
    if head in ("codex", "claude"):
        if args and args[0] in _MANAGEMENT[head]:
            return False
        return not (len(args) == 1 and args[0] in _INFO_FLAGS)
    return False

def _refusal(event):
    return SUBAGENT_MSG if is_dispatched(event) else MSG % resolve(event)

def main():
    event = read_event()
    if _chooses_fable(event):
        return feedback.block("block_spawning", FABLE_MSG)
    if _types_orchestrate(event):
        return feedback.block("block_spawning", ORCHESTRATE_MSG)
    if may_spawn(event):
        return 0
    if canonical_tool(event) == "agent":
        return feedback.block("block_spawning", _refusal(event))
    found = command.invocations(command_str(event))
    if canonical_tool(event) == "shell" and (found is None or any(_spawns(head, args) for head, args in found)):
        return feedback.block("block_spawning", _refusal(event))
    return 0

if __name__ == "__main__":
    sys.exit(main())
