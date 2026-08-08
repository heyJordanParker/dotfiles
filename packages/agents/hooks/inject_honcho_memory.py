#!/usr/bin/env python3
"""Inject the memory a turn should start from.

Two collections, because they answer two different questions. The architect's
own holds who he is and how he works, and every agent needs it. The running
agent's holds what that agent has said and been told, and only it needs that.
They are disjoint by construction — nobody observes anybody, so each collection
holds conclusions about its own peer's messages and nothing appears twice.

An agent working on its own gets both as well, on the brief it was handed: its
collection exists to be read by it, and keying the search off the architect's
words left every per-agent collection written and never read — a Claude subagent
was skipped outright, and a codex run has no architect in its transcript at all,
so both came back empty.

A Claude subagent is reached through its dispatch, not through its own turn: no
prompt event fires inside one, verified by asking a dispatched agent whether it
saw a memory block and getting NONE. The dispatch is a tool call, so the memory
goes into the brief itself — the one text a subagent is guaranteed to read — by
rewriting the call's input the way `name_memory_caller.py` does. Which peer that
is comes off the dispatch payload, so it is the agent about to run, never the one
dispatching it.

The brief is used only when it is not machine-authored. A task notification
arrives on the same field, and retrieving against one is what made the plugin
spend a turn's budget answering its own telemetry.

Two moments, because memory is needed at each and available differently:
- SessionStart, unsearched: the session opens with what each collection holds
  most strongly. Without it a session starts blank until a prompt happens to
  match something. Compaction fires this event too, with `source: compact`, so
  the moment the conversation is replaced by a summary — when memory is thinnest
  and matters most — is covered from the far side of the compaction.
- UserPromptSubmit, searched on the turn's own words.

Not PreCompact. It is an input-only event: Claude's output schema has no variant
for it, so the block was rejected wholesale and the architect saw a validation
banner instead of memory. SessionStart is the channel that reaches the same
moment, and `lib/feedback.CONTEXT_EVENTS` now holds which events can carry text
at all.

An agent declaring `memory: none` gets nothing. That declaration is about the
agent, not about the tool it reaches memory through, so it holds on the way in
as well as on the way out.
"""

import re
import sys

from lib import agent_memory, feedback, honcho, transcript
from lib.event import agent_name, field, read_event

BINDING = {
    # Retrievals run in front of the turn, so this budget is felt directly. Each
    # is capped at RETRIEVAL_TIMEOUT and there are at most three — the card and
    # both collections — and the hook's own ceiling has to clear their sum or a
    # slow server truncates the memory at whichever one it reached.
    "events": {"UserPromptSubmit": [], "SessionStart": [], "PreToolUse": ["Agent"]},
    "timeout": 20,
    "harness": "all",
    "roots": "all",
}

# Well under lib/honcho.TIMEOUT, which governs writes that nobody waits on. A
# retrieval that has not answered by now is worth abandoning for the cached one:
# the turn starts sooner and still starts with memory.
RETRIEVAL_TIMEOUT = 4

# The event that carries no prompt: memory is fetched unsearched, and the server
# answers with each collection's own strongest conclusions.
UNSEARCHED = ("SessionStart",)

# Prompts that cannot inform a retrieval — an acknowledgement, or a slash command
# the harness is about to expand. The plugin skipped these; without the skip
# every "ok" spent two network calls to search on the word "ok".
TRIVIAL = re.compile(r"^\s*(?:/\S*|y|n|ok(?:ay)?|yes|no|sure|thanks|thank you|ty|"
                     r"go|do it|continue|next|k)\s*[.!]*\s*$", re.IGNORECASE)


def query_text(event):
    """What this turn is about: the architect's words, else the agent's brief.

    A subagent goes straight to the brief. Its payload carries the *parent's*
    transcript, so the architect's message found there belongs to the turn that
    dispatched it, not to this one.
    """
    if not field(event, "agent_id", ""):
        architect = transcript.architect_message(
            transcript.records(field(event, "transcript_path", "")))
        if architect.strip():
            return architect
    brief = field(event, "prompt", "")
    return "" if transcript.harness_authored(brief) else brief


def block(peer, lines):
    return "[Honcho Memory for %s]: %s" % (peer, "; ".join(lines))


def main():
    event = read_event()
    event_name = field(event, "hook_event_name", "") or "UserPromptSubmit"
    unsearched = event_name in UNSEARCHED
    dispatch = event_name == "PreToolUse"

    cfg = honcho.config()
    if not honcho.enabled(cfg):
        return 0

    if dispatch:
        agent = field(event, "tool_input.subagent_type", "")
        text = field(event, "tool_input.prompt", "")
        if not agent or not text.strip():
            return 0
    else:
        agent = agent_name(event)
        text = "" if unsearched else query_text(event)
        if not unsearched and (not text.strip() or TRIVIAL.match(text)):
            return 0

    if agent and agent_memory.denies_memory(agent_memory.definition_path(agent)):
        return 0

    blocks = []
    for peer in (cfg.get("peerName"), agent):
        if not peer:
            continue
        lines = honcho.remembered_context(cfg, peer, query=text, timeout=RETRIEVAL_TIMEOUT)
        if peer == cfg.get("peerName"):
            lines = honcho.card(cfg, peer, timeout=RETRIEVAL_TIMEOUT) + lines
        if lines:
            blocks.append(block(peer, lines))

    if not blocks:
        return 0

    if dispatch:
        tool_input = dict(field(event, "tool_input", {}) or {})
        tool_input["prompt"] = "%s\n\n%s" % (
            feedback.wrap("inject_honcho_memory", "\n\n".join(blocks)), text)
        return feedback.updated_input(event_name, tool_input)

    feedback.context("inject_honcho_memory", event_name, "\n\n".join(blocks))
    return 0


if __name__ == "__main__":
    sys.exit(main())
