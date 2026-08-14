"""Contract for the canonical cross-harness tool map (lib/event.canonical_tool).

This is our owned translation from each harness's emitted tool_name to one
canonical name. What is pinned here is the unmapped case: a tool_name the table
does not hold, an empty one, and a payload carrying none at all each answer ""
rather than guessing a canonical name a gate would then act on.
"""

import pytest
from lib.event import canonical_tool


@pytest.mark.parametrize("payload", [
    {"tool_name": "WebFetch"},          # unmapped tool
    {"tool_name": ""},                  # empty
    {},                                 # no tool_name at all
])
def test_unmapped_tool_is_empty(payload):
    assert canonical_tool(payload) == ""
