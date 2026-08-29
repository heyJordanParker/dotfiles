"""Contract for the canonical cross-harness tool map (lib/event.canonical_tool).

This is our owned translation from each harness's emitted tool_name to one
canonical name. What is pinned here is the unmapped case: a tool_name the table
does not hold, an empty one, and a payload carrying none at all each answer ""
rather than guessing a canonical name a gate would then act on.
"""

import pytest
from lib.event import canonical_tool, command_str, patch_target, patch_text

PATCH = (
    "*** Begin Patch\n"
    "*** Update File: app/x.py\n"
    "@@\n"
    "-a = 1\n"
    "+a = 2\n"
    "*** End Patch\n"
)


@pytest.mark.parametrize("payload", [
    {"tool_name": "WebFetch"},          # unmapped tool
    {"tool_name": ""},                  # empty
    {},                                 # no tool_name at all
])
def test_unmapped_tool_is_empty(payload):
    assert canonical_tool(payload) == ""


def test_a_codex_patch_carries_no_shell_command():
    """codex sends the patch body under the same `command` key the shell tool
    uses, so every command guard read the diff's own text as commands."""
    event = {"tool_name": "apply_patch", "tool_input": {"command": PATCH}}
    assert command_str(event) == ""


def test_a_shell_call_still_carries_its_command():
    assert command_str({"tool_name": "Bash", "tool_input": {"command": "ls"}}) == "ls"
    assert command_str({"tool_name": "Bash", "tool_input": {
        "command": ["/bin/zsh", "-lc", "ls"]}}) == "/bin/zsh -lc ls"


@pytest.mark.parametrize("payload", [
    {"tool_input": {"command": "rm -rf /"}},                  # no tool_name
    {"tool_name": "mcp__x__run", "tool_input": {"command": "rm -rf /"}},
])
def test_an_unidentified_tool_is_still_judged(payload):
    """Reading nothing off a tool the table does not hold would pass every
    command guard on a line they cannot identify."""
    assert command_str(payload) == "rm -rf /"


def test_a_patch_names_the_file_it_touches():
    event = {"tool_name": "apply_patch", "tool_input": {"command": PATCH}}
    assert patch_text(event) == PATCH
    assert patch_target(event) == "app/x.py"


def test_a_claude_write_names_its_own_file():
    event = {"tool_name": "Write", "tool_input": {"file_path": "app/y.py"}}
    assert patch_target(event) == "app/y.py"
    assert patch_text(event) == ""
