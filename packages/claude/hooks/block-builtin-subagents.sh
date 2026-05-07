#!/bin/bash
# Block built-in subagents that default to non-Opus models, redirecting to user-defined Opus equivalents.
# Also blocks tool_input.model overrides that aren't "opus".
# Gracefully allows on any error.

read -r input

subagent_type=$(echo "$input" | jq -r '.tool_input.subagent_type // ""' 2>/dev/null) || exit 0

case "$subagent_type" in
    Explore)
        cat >&2 <<'EOF'
BLOCKED: Built-in Explore is replaced by the researcher agent.

Explore runs on Haiku 4.5 and returns excerpts that miss content past its read window — unreliable for review, audits, or any open-ended analysis.
Researcher runs on Opus, reads whole files, and returns structured findings with file:line citations. Its scope covers both in-codebase exploration ("where is X defined / which files reference Y") and external research.

Set subagent_type: researcher. Brief it with WHY and WHAT — the question or goal — not HOW (which files to read or commands to run).
EOF
        exit 2
        ;;
    general-purpose)
        cat >&2 <<'EOF'
BLOCKED: Built-in general-purpose is replaced by specialized Opus agents.

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

Set subagent_type to one of the above. Brief with Story / Business / Goal / DoD per /subagents.
EOF
        exit 2
        ;;
    Plan)
        cat >&2 <<'EOF'
BLOCKED: Built-in Plan is replaced by the architect agent.

Architect runs on Opus, applies strict encapsulation and dependency-direction analysis, reads the project's Claude.md conventions, and presents multiple scored options with pros/cons instead of one path. It stays read-only — describes WHAT to change and WHERE, leaves implementation to execution agents.

Set subagent_type: architect. Brief it with the user-facing problem (WHY/WHO) and the scope; let it map the codebase itself.
EOF
        exit 2
        ;;
    claude-code-guide)
        cat >&2 <<'EOF'
BLOCKED: Built-in claude-code-guide is replaced by the context-engineer agent.

context-engineer runs on Opus, loads the cc and claude-api skills, and works against this repo's actual Claude.md / hooks / skills / plugin layout — answers fit our conventions and existing patterns instead of generic Claude Code advice.

Set subagent_type: context-engineer. State the question or the change you want made; it'll read the relevant Claude.md hierarchy itself.
EOF
        exit 2
        ;;
esac

# Block model overrides that aren't opus on user-defined agents
model_override=$(echo "$input" | jq -r '.tool_input.model // ""' 2>/dev/null) || model_override=""

if [ -n "$model_override" ] && [ "$model_override" != "opus" ]; then
    cat >&2 <<EOF
BLOCKED: tool_input.model is set to "$model_override".

User-defined agents declare model: opus in their frontmatter. Overriding to a cheaper model defeats the agent's design and produces lazy output.
Remove tool_input.model from the dispatch, or set it to "opus".
EOF
    exit 2
fi

exit 0
