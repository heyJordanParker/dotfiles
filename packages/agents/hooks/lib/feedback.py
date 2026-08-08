"""Wrap every hook's message to the agent in a name tag.

The output mirror of lib/event.py: event.py parses what the harness sends a hook;
feedback.py is the single owner of what a hook says back. The agent reads hook
text in the same stream as the architect's words, so every message is wrapped in
<{name}_agent>…</{name}_agent> — the agent can always tell an automated hook from
the architect. The tag carries no instructions; how to treat tagged text lives
once in the global rules.

classify_intent.is_system_prompt already recognizes this <tag>…</tag> shape and
keeps it out of user-intent classification, so the same tag that marks hook-voice
to the agent also keeps a hook's words from being read as the user's.

Four channels:
- block(name, body): a gate's stderr message; returns 2, which HALTS the stop or
  tool and re-fires — the agent is forced to act. For genuine hard stops only.
- raise_concern(name, event_name, body): surfaces a concern WITHOUT halting —
  `systemMessage` shows it to the architect, `additionalContext` gives it to the
  agent, and the return is 0 so the stop/tool proceeds. A validator FLAGS; the
  agent holds the context and decides. This is the validators' channel.
- context(name, event_name, body): injected context as additionalContext.
- updated_input(event_name, tool_input): replaces the tool call's own input, for
  a fact the hook holds and the tool cannot reach. It carries no
  `permissionDecision`, so it grants nothing another gate would refuse; verified
  live on Bash that the replacement is what runs.
"""

import json
import sys

# The events whose `hookSpecificOutput` carries `additionalContext`. Claude
# validates that object against a union keyed on `hookEventName` and rejects the
# whole output when the event has no variant — the hook then shows the architect a
# validation banner and injects nothing, which is how `PreCompact` injection died
# silently for weeks. Read from the harness binary at 2.1.226; `PreCompact`,
# `PostCompact`, `SessionEnd` and `StopFailure` are input-only events with no
# output variant at all.
CONTEXT_EVENTS = (
    "PreToolUse", "PostToolUse", "PostToolUseFailure", "PostToolBatch",
    "UserPromptSubmit", "UserPromptExpansion", "SessionStart", "Setup",
    "SubagentStart", "Stop", "SubagentStop", "Notification",
)


def wrap(name, body):
    return "<%s_agent>\n%s\n</%s_agent>" % (name, body, name)


def block(name, body):
    sys.stderr.write(wrap(name, body) + "\n")
    return 2


def _carries_context(name, event_name):
    """Whether this event can carry text back, saying so when it cannot.

    A hook bound to an event with no output variant is a binding mistake, and the
    honest place to see it is here rather than in a validation banner that also
    throws away everything the hook did."""
    if event_name in CONTEXT_EVENTS:
        return True
    sys.stderr.write("%s: %s carries no additionalContext; nothing injected\n"
                     % (name, event_name))
    return False


def raise_concern(name, event_name, body):
    wrapped = wrap(name, body)
    if not _carries_context(name, event_name):
        return 0
    sys.stdout.write(json.dumps(
        {"systemMessage": wrapped,
         "hookSpecificOutput": {
             "hookEventName": event_name,
             "additionalContext": wrapped,
         }},
        separators=(",", ":"), ensure_ascii=False,
    ) + "\n")
    return 0


def context(name, event_name, body):
    if not _carries_context(name, event_name):
        return 0
    sys.stdout.write(json.dumps(
        {"hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": wrap(name, body),
        }},
        separators=(",", ":"), ensure_ascii=False,
    ) + "\n")
    return 0


def updated_input(event_name, tool_input):
    # PreToolUse is the only variant carrying `updatedInput`; anywhere else the
    # whole output is rejected and the tool call runs unchanged regardless.
    if event_name != "PreToolUse":
        sys.stderr.write("%s cannot replace a tool input; nothing rewritten\n" % event_name)
        return 0
    sys.stdout.write(json.dumps(
        {"hookSpecificOutput": {
            "hookEventName": event_name,
            "updatedInput": tool_input,
        }},
        separators=(",", ":"), ensure_ascii=False,
    ) + "\n")
    return 0
