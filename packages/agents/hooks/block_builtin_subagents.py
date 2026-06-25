#!/usr/bin/env python3
"""Redirect built-in subagents to Opus equivalents; block non-opus model overrides."""

import sys

from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["Agent"]},
    "timeout": 5,
    "harness": "claude",
}

EXPLORE_MSG = """BLOCKED: Built-in Explore is replaced by the explorer agent.

Explore runs on Haiku 4.5 and returns excerpts that miss content past its read window — unreliable for review, audits, or any open-ended analysis.
explorer runs on Opus 4.8 with the trace skill (a code-intelligence CLI that surfaces complexity, callers, dependencies, git lifecycle, deploy-branch presence, and nested Claude.md context per file). It reads whole files, categorizes findings by impact (load-bearing / moderate / minor), and returns a structured five-section trace report with verified file:line citations.

Use it for "where is X used", "how does Y work end-to-end", "what depends on Z", or any question that needs the agent to map connections between files, modules, or layers. For external research (library docs, API references), use the researcher agent instead.

Set subagent_type: explorer. Brief it with WHY and WHAT — the question or goal — not HOW (which files to read or commands to run)."""

GENERAL_MSG = """BLOCKED: Built-in general-purpose is replaced by specialized Opus agents.

A specialist gives better domain framing, the right tool set, and Opus-level reasoning. Match by task:

  architect          — system design, encapsulation, dependency review (read-only)
  backend-engineer   — backend implementation, API correctness, regression checks
  frontend-engineer  — frontend implementation + user flow verification
  designer           — UI components, CSS, visual implementation
  code-reviewer      — diff slop scanning (over-defense, dead code, silent failures)
  debugger           — bug investigation, root cause tracing (read-only)
  researcher         — external research and in-codebase lookups (read-only)
  tester             — feature verification: API curls, UI walks, flow tracing (read-only)
  ux-tester          — pure user-perspective UX walkthroughs (no code reading)
  context-engineer   — Claude.md maintenance, hooks, skills, plugin work
  codex              — faster but overengineers code; great for research and quick prototypes

Set subagent_type to one of the above. Brief with Story / Business / Goal / DoD per /subagents."""

PLAN_MSG = """BLOCKED: Built-in Plan is replaced by the architect agent.

Architect runs on Opus, applies strict encapsulation and dependency-direction analysis, reads the project's Claude.md conventions, and presents multiple scored options with pros/cons instead of one path. It stays read-only — describes WHAT to change and WHERE, leaves implementation to execution agents.

Set subagent_type: architect. Brief it with the user-facing problem (WHY/WHO) and the scope; let it map the codebase itself."""

GUIDE_MSG = """BLOCKED: Built-in claude-code-guide is replaced by the context-engineer agent.

context-engineer runs on Opus, loads the cc and claude-api skills, and works against this repo's actual Claude.md / hooks / skills / plugin layout — answers fit our conventions and existing patterns instead of generic Claude Code advice.

Set subagent_type: context-engineer. State the question or the change you want made; it'll read the relevant Claude.md hierarchy itself."""

_BY_TYPE = {
    "Explore": EXPLORE_MSG,
    "general-purpose": GENERAL_MSG,
    "Plan": PLAN_MSG,
    "claude-code-guide": GUIDE_MSG,
}


def main():
    event = read_event()
    subagent_type = field(event, "tool_input.subagent_type", "")
    msg = _BY_TYPE.get(subagent_type)
    if msg is not None:
        sys.stderr.write(msg + "\n")
        return 2

    model_override = field(event, "tool_input.model", "")
    if model_override and model_override != "opus":
        sys.stderr.write(
            'BLOCKED: tool_input.model is set to "%s".\n\n'
            "User-defined agents declare model: opus in their frontmatter. Overriding to a cheaper model defeats the agent's design and produces lazy output.\n"
            'Remove tool_input.model from the dispatch, or set it to "opus".\n' % model_override
        )
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
