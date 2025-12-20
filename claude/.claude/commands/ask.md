---
description: Break complex scenarios into self-contained decision questions with 4+ options each
---

# /ask

For complex multi-decision scenarios. Each decision = one AskUserQuestion call.

**REQUIRED SKILL:** Use `asking-decisions` skill for question structure.

## When to Use
- 3+ independent decisions needing approval
- Claude.md / docs overhauls
- Architecture spanning multiple concerns
- Anything too complex for a single plan review

## Process

1. Identify all decision points
2. Research each (codebase, docs) until ≥85% confident
3. Skip decisions where you're ≥85% confident - just decide
4. For each <85% decision: use AskUserQuestion with 4+ options
5. One question at a time
6. Embed full context IN the question (user shouldn't need to read anything else)
