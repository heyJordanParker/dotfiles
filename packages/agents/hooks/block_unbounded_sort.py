#!/usr/bin/env python3
"""Refuse a search whose output is not bounded piped into sort or uniq.

sort holds its whole input in memory. `rg -o` over the transcript corpus emitted
5.26 GB and sort peaked at 4.56x its input, so four concurrent censuses became four
10 GB processes. The bound on the search makes the pipeline safe, not the size of
the tree.
"""

import sys

from lib import feedback
from lib.command import command_head, head_and_args, is_separator, script_lines, tokenize
from lib.event import command_str, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

MEMORY_SINKS = {"sort", "uniq"}
SEARCHERS = {"rg", "grep", "egrep", "fgrep", "ag", "find", "fd"}
BOUNDING_FLAGS = ("-c", "--count", "--count-matches", "-m", "--max-count",
                  "-l", "--files-with-matches")

MESSAGE = """BLOCKED: an unbounded search piped into sort holds the whole stream in memory.

`rg -o` over a large tree emits gigabytes, and sort costs about 4.5x its input
in resident memory. Bound the search instead:
  rg -c <pattern> <path>                     counts, materializing nothing
  rg --max-count <n> <pattern> <path>
  rg <pattern> <path> | head -200 | sort"""


def _pipes(line):
    """Each command of a line paired with the command its output is piped into."""
    commands, words = [], []
    for token in tokenize(line) or []:
        if is_separator(token):
            commands.append((words, token))
            words = []
        else:
            words.append(token)
    commands.append((words, ""))
    return [(words, commands[i + 1][0])
            for i, (words, operator) in enumerate(commands[:-1])
            if operator in ("|", "|&")]


def _unbounded_search(words):
    head, args = head_and_args(words)
    return head in SEARCHERS and not any(
        arg == flag or arg.startswith(flag + "=") for arg in args for flag in BOUNDING_FLAGS)


def main():
    command = command_str(read_event())
    for line in script_lines(command) or []:
        for source, sink in _pipes(line):
            if command_head(sink) in MEMORY_SINKS and _unbounded_search(source):
                return feedback.block("block_unbounded_sort", MESSAGE)
    return 0


if __name__ == "__main__":
    sys.exit(main())
