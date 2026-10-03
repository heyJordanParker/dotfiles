#!/usr/bin/env python3
"""Inject the memory a turn should start from.

Each project is a Honcho scope, and its peer `scope.<project>` keeps a view of
everyone who spoke in its sessions. A turn reads one block per peer, the running
agent on its model first, then the architect: the lines of the project's scope
about that peer that match the turn, then the lines of the peer's own view that
hold in every project. The agent's block needs its model, which a Claude
conversation records only with its first reply, so a first turn reads the
architect's block alone.

An agent working on its own gets them as well, on the brief it was handed: its
memory exists to be read by it, and keying the search off the architect's
words left every per-agent memory written and never read — a Claude subagent
was skipped outright, and a codex run has no architect in its transcript at all,
so both came back empty.

A Claude subagent is reached through its dispatch, not through its own turn: no
prompt event fires inside one, verified by asking a dispatched agent whether it
saw a memory block and getting NONE. The dispatch is a tool call, so the memory
goes into the brief itself — the one text a subagent is guaranteed to read — by
rewriting the call's input through `updatedInput`. Which peer that is comes off
the dispatch payload, so it is the agent about to run, never the one
dispatching it, read on the dispatching conversation's model.

The brief is used only when it is not machine-authored. A task notification
arrives on the same field, and retrieving against one is what made the plugin
spend a turn's budget answering its own telemetry.

Two moments, because memory is needed at each and available differently:
- SessionStart: the scope's card of the architect, the patterns Honcho's dream
  drew about each peer in the project, and the architect's newest deliberate
  saves. Compaction fires this event too, with `source: compact`, so it is back
  on the far side. Not the newest conclusions: those belong to whichever
  conversation ran last.
- UserPromptSubmit and a dispatch brief: each peer's project lines and general
  lines that match the turn's own words, and nothing when none match.

Not PreCompact. It is an input-only event: Claude's output schema has no variant
for it, so the block was rejected wholesale and the architect saw a validation
banner instead of memory. SessionStart is the channel that reaches the same
moment, and `lib/feedback.CONTEXT_EVENTS` now holds which events can carry text
at all.

A read Honcho does not answer in time is left out, so a turn starts without that
memory rather than late.

An agent declaring `memory: none` gets nothing. That declaration is about the
agent, not about the tool it reaches memory through, so it holds on the way in
as well as on the way out.
"""

import os
import re
import sys
from concurrent.futures import ThreadPoolExecutor

from lib import agent_memory, feedback, honcho, transcript
from lib.event import field, read_event

BINDING = {
    # Retrievals run in front of the turn, so this budget is felt directly. They
    # run at once, each capped at RETRIEVAL_TIMEOUT, and the hook's own ceiling
    # clears that with room for the transcript read and the git lookups.
    "events": {"UserPromptSubmit": [], "SessionStart": [], "PreToolUse": ["Agent"]},
    "timeout": 20,
    "harness": "all",
    "standalone": True,
    "roots": "all",
}

# Well under lib/honcho.TIMEOUT, which governs writes that nobody waits on.
# Honcho answers a read in 0.35-0.40 s, so a read still open at this
# point is abandoned and the turn starts without it.
RETRIEVAL_TIMEOUT = 4

# The event that carries no prompt: it reads the distilled patterns instead of a search.
UNSEARCHED = ("SessionStart",)

# Prompts that cannot inform a retrieval — an acknowledgement, or a slash command
# the harness is about to expand. The plugin skipped these; without the skip
# every "ok" spent two network calls to search on the word "ok".
TRIVIAL = re.compile(r"^\s*(?:/\S*|y|n|ok(?:ay)?|yes|no|sure|thanks|thank you|ty|"
                     r"go|do it|continue|next|k)\s*[.!]*\s*$", re.IGNORECASE)


def query_text(event):
    """What this turn is about: its prompt when the Harness did not write it."""
    brief = field(event, "prompt", "")
    return "" if transcript.harness_authored(brief) else brief


def section(heading, answer):
    """One read's lines under `heading`, or "" when it held none."""
    return "## %s\n\n%s" % (heading, "\n".join(answer)) if answer else ""


def block(peer, sections):
    """One peer's memory under one header, or "" when no read held any."""
    body = "\n\n".join(text for text in sections if text)
    return "[Honcho Memory about %s]:\n%s" % (peer, body) if body else ""


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
        if not os.path.isfile(agent_memory.definition_path(agent)):
            agent = honcho.memory_agent(event)
    else:
        agent = honcho.memory_agent(event)
        text = "" if unsearched else query_text(event)
        if not unsearched and (not text.strip() or TRIVIAL.match(text)):
            return 0

    if agent and agent_memory.denies_memory(agent_memory.definition_path(agent)):
        return 0

    architect = cfg.get("peerName", "")
    model = honcho.event_model(event, transcript.records(field(event, "transcript_path", "")))
    agent_peer = honcho.peer_name(agent, model)
    project = honcho.project_name(field(event, "cwd", "") or os.getcwd())
    if unsearched:
        reads = [(agent_peer, [("Patterns", honcho.patterns, agent_peer, project)]),
                 (architect, [("Peer Card", honcho.card, project, architect),
                              ("Patterns", honcho.patterns, architect, project),
                              ("General", honcho.general, architect)])]
    else:
        reads = [(peer, [("Project", honcho.search, peer, project, text),
                         ("General", honcho.general, peer, text)])
                 for peer in (agent_peer, architect)]
    with ThreadPoolExecutor(max_workers=4) as pool:
        pending = [(peer, [(heading, pool.submit(read, cfg, *args, timeout=RETRIEVAL_TIMEOUT))
                           for heading, read, *args in peer_reads])
                   for peer, peer_reads in reads if peer]
    blocks = [block(peer, [section(heading, future.result()) for heading, future in futures])
              for peer, futures in pending]
    blocks = [text for text in blocks if text]

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
