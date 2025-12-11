---
description: Review Claude documentation (skills, commands, Claude.md) with 2 parallel subagents
---

Review Claude documentation using 2 specialized reviewers in parallel.

## Instructions

Launch these 2 subagents in parallel using the Task tool. Each subagent reviews all changes and all skill/command files.

### Subagent 1: Documentation Style Review

```
You are a documentation reviewer. Use the `updating-claude-documentation` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the updating-claude-documentation skill
3. Find and read all skill and command files
4. Review for:
   - Style guide compliance
   - Proper hierarchy placement
   - Contradictions between files
   - Duplication that shouldn't exist

Report findings:

**Critical:** (contradicts existing docs, wrong hierarchy level)
**Important:** (style violations, unnecessary duplication)
**Minor:** (suggestions for clarity)

If no issues: "No documentation style issues found."
```

### Subagent 2: Context Efficiency Review

```
You are a context engineer. Use the `context-engineering` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the context-engineering skill
3. Find and read all skill and command files
4. Review for:
   - Context efficiency (too verbose? too sparse?)
   - Hedging language ("should", "might", "consider")
   - Decorative bloat (fancy boxes, redundant tables)
   - Instructions that don't prevent specific mistakes

Report findings:

**Critical:** (major context bloat, unclear instructions)
**Important:** (hedging language, redundant sections)
**Minor:** (formatting suggestions)

If no issues: "No context efficiency issues found."
```

## After All Subagents Complete

Aggregate results into a unified report:

```
# Documentation Review Results

## Critical (must fix)
[List all critical issues from both reviewers]

## Important (should fix)
[List all important issues from both reviewers]

## Minor (consider)
[List all minor issues from both reviewers]

---
Reviewed by: documentation-style, context-efficiency
```

If both reviewers report no issues: "All clear. Documentation looks good."
