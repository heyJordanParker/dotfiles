---
name: plan
description: Use when planning implementation - either writing detailed plans for engineers or structuring complex decisions via questions.
---

# Plan

Framework for implementation planning.

## Triggers

- Design is complete, need implementation tasks
- Complex multi-decision scenarios
- Need to clarify approach before building

## Planning Modes

- **writing-plans** — [writing-plans.md](references/writing-plans.md) — create detailed task list for implementation
- **question-driven** — [question-driven.md](references/question-driven.md) — multiple decisions needing approval

## Quick Reference

### Writing Plans

Save to: `docs/plans/YYYY-MM-DD-<feature-name>.md`

**Task structure:**
1. Files (create/modify/test paths)
2. Step: Write failing test
3. Step: Verify it fails
4. Step: Write minimal implementation
5. Step: Verify it passes
6. Step: Commit

**Key rules:**
- Exact file paths always
- Complete code in plan
- Bite-sized tasks (2-5 min each)
- DRY, YAGNI, TDD

### Question-Driven

**Format per question:**
- Architecture tree (affected files marked)
- Context (1-2 sentences)
- Current state (code snippet)
- Proposed change (code snippet)
- Risk (1 sentence)
- 4+ options via AskUserQuestion

**Rules:**
- Self-contained (no external docs needed)
- Research first (only ask if <85% confident)
- One question at a time

## Process

1. **Identify mode** - tasks or decisions?
2. **Read relevant reference**
3. **Follow format exactly**
