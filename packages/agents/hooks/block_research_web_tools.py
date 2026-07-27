#!/usr/bin/env python3
"""Force the researcher agent's web reads through /agent-browser.

The researcher browses the open web for buyer language and competitor pages. Every
URL it visits must be logged to the source registry (scripts/sources.py log), and
that logging is an agent step inside the research skills' agent-browser contract —
so a direct WebFetch or WebSearch is a read that escapes the registry entirely.
This gate closes that escape: the researcher reaches the web through agent-browser
and nothing else.

The block is scoped to `researcher` by name, not to research work in general. The
reality-reviewer's check-reality contract REQUIRES direct WebFetch and WebSearch —
it fetches the cited URL for every element-2 quote and searches claimed venues to
spot-check that quotes resolve verbatim in the outside world. Blocking its fetches
would break citation verification, so it keeps them; only the researcher is routed
through the browser.

`agent_id` is the presence test for "inside a subagent" — matching block_memory_
access.py, the field that means that and nothing else. `agent_type` names which
agent is running; the block fires only for `researcher`. WebFetch and WebSearch are
Claude tool names, so this binds to the Claude harness; on codex the researcher has
no such tools and reaches the web through agent-browser already.
"""

import json
import sys

from lib import feedback
from lib.event import field

BINDING = {
    "events": {"PreToolUse": ["WebFetch|WebSearch"]},
    "timeout": 5,
    "harness": "claude",
    "roots": "all",
}

MSG = """BLOCKED: the researcher reaches the web through /agent-browser only.

%s is a direct web read that escapes the source registry. Every URL you visit
must be logged with `python3 <profile>/scripts/sources.py log <url>`, and that
logging lives in the agent-browser step of the research skills. A direct fetch or
search skips it.

Open the page through agent-browser instead (`agent-browser skills get core`),
and log its URL. Do not retry through another web tool."""


def main():
    try:
        event = json.load(sys.stdin)
    except Exception:
        print("block_research_web_tools: malformed input: invalid JSON", file=sys.stderr)
        return 0
    if not isinstance(event, dict):
        print("block_research_web_tools: malformed input: expected a JSON object", file=sys.stderr)
        return 0
    if not field(event, "tool_name", ""):
        print("block_research_web_tools: malformed input: missing tool_name", file=sys.stderr)
        return 0
    tool = field(event, "tool_name", "")
    if tool not in ("WebFetch", "WebSearch"):
        return 0
    if not field(event, "agent_type", ""):
        # The main thread carries no agent_type; only an event that claims to be
        # a subagent (agent_id present) without naming its type is malformed.
        if field(event, "agent_id", ""):
            print("block_research_web_tools: malformed input: missing agent_type", file=sys.stderr)
        return 0
    if field(event, "agent_type", "") != "researcher":
        return 0
    # No agent_id check: a session launched directly AS the researcher has the
    # agent_type but no agent_id, and it is exactly as bound by the browser rule.
    return feedback.block("block_research_web_tools", MSG % tool)


if __name__ == "__main__":
    sys.exit(main())
