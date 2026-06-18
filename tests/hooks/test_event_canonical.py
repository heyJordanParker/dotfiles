"""Contract for the canonical cross-harness tool map (lib/event.canonical_tool).

This is our owned translation from each harness's emitted tool_name to one
canonical name. The point of the test is that the contract is OURS: if a harness
changes the tool_name it emits, this fails loudly rather than a hook silently
ceasing to match. We never depend on codex's own Claude-compat aliasing being
correct — we assert every name we expect, from both harnesses.
"""

import pytest
from lib.event import canonical_tool


@pytest.mark.parametrize("emitted,expected", [
    # shell — Claude's Bash, codex's native shell tools, and codex's compat alias
    ("Bash", "shell"),
    ("shell_command", "shell"),
    ("exec_command", "shell"),
    # file read — Claude only has a read tool; codex reads via the shell
    ("Read", "read"),
    # file write/edit — Claude's edit tools, codex's apply_patch
    ("Write", "write"),
    ("Edit", "write"),
    ("MultiEdit", "write"),
    ("apply_patch", "write"),
    # subagent dispatch — Claude's Agent, codex's spawn_agent
    ("Agent", "agent"),
    ("spawn_agent", "agent"),
    # ask the user — Claude's question tool, codex's request_user_input
    ("AskUserQuestion", "ask"),
    ("request_user_input", "ask"),
])
def test_emitted_name_maps_to_canonical(emitted, expected):
    assert canonical_tool({"tool_name": emitted}) == expected


@pytest.mark.parametrize("payload", [
    {"tool_name": "WebFetch"},          # unmapped tool
    {"tool_name": ""},                  # empty
    {},                                 # no tool_name at all
])
def test_unmapped_tool_is_empty(payload):
    assert canonical_tool(payload) == ""
