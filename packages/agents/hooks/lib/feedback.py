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

Three channels:
- block(name, body): a gate's stderr message; returns 2, which HALTS the stop or
  tool and re-fires — the agent is forced to act. For genuine hard stops only.
- raise_concern(name, event_name, body): surfaces a concern WITHOUT halting —
  `systemMessage` shows it to the architect, `additionalContext` gives it to the
  agent, and the return is 0 so the stop/tool proceeds. A validator FLAGS; the
  agent holds the context and decides. This is the validators' channel.
- context(name, event_name, body): injected context as additionalContext.
"""

import json
import sys


def wrap(name, body):
    return "<%s_agent>\n%s\n</%s_agent>" % (name, body, name)


def block(name, body):
    sys.stderr.write(wrap(name, body) + "\n")
    return 2


def raise_concern(name, event_name, body):
    wrapped = wrap(name, body)
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
    sys.stdout.write(json.dumps(
        {"hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": wrap(name, body),
        }},
        separators=(",", ":"), ensure_ascii=False,
    ) + "\n")
