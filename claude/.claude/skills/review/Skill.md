---
name: review
description: Use for code review - runs all reviewers in parallel on uncommitted changes
---

# Review

Full code review gate. Runs 1 parallel subagent + 6 agents on uncommitted changes.

## Current Changes

!`git changes`

## Full Diff

!`git diff HEAD`

## Instructions

Include the "Current Changes" and "Full Diff" sections above in each subagent prompt instead of telling them to run `git diff HEAD`.

Launch 7 reviewers in parallel: 1 subagent using the Task tool + 6 agents using the Agent tool (`subagent_type: "code-reviewer"`, `subagent_type: "architect"`, `subagent_type: "backend-engineer"`, `subagent_type: "frontend-engineer"`, `subagent_type: "context-engineer"`, and `subagent_type: "ux-tester"`).

### Code-Reviewer Agent: Anti-Slop

Dispatch using `subagent_type: "code-reviewer"` via the Agent tool (not Task tool). Prompt:

```
Review uncommitted changes. Review the diff provided and scan all 12 slop categories. Report using Critical/Important/Minor format. If clean: "No slop found."
```

### Architect Agent

Dispatch using `subagent_type: "architect"` via the Agent tool (not Task tool). Prompt:

```
Review uncommitted changes. Review the diff provided and apply your full review protocol. Report using Critical/Important/Minor format.
```

### Subagent 3: Naming

```
You are a naming reviewer. Read references/naming.md for guidance.

1. Review the diff provided
2. Check: misleading names, generic names, convention violations, abbreviation abuse
3. Report:

**Critical:** (naming that causes confusion or bugs)
**Important:** (naming that hurts readability)
**Minor:** (naming suggestions)

If clean: "No naming issues found."
```

### Backend-Engineer Agent: Simplicity & Elegance

Dispatch using `subagent_type: "backend-engineer"` via the Agent tool (not Task tool). Prompt:

```
Review uncommitted changes. Review the diff provided and apply your review mode protocol. Check: reinvented wheels, library leverage, YAGNI, complexity creep, approach quality, API regressions. Report using Critical/Important/Minor format. If clean: "Code is appropriately simple."
```

### Frontend-Engineer Agent: User Flows

Dispatch using `subagent_type: "frontend-engineer"` via the Agent tool (not Task tool). Prompt:

```
Review uncommitted changes. Review the diff provided and apply your user flow testing protocol. Identify affected user flows, trace each one through the code, and report gaps. Report using Critical/Important/Minor format. If clean: "All user flows verified."
```

### Context-Engineer Agent: Ledger

Dispatch using `subagent_type: "context-engineer"` via the Agent tool (not Task tool). Prompt:

```
Audit Claude.md files against the current uncommitted changes (diff provided). Report using Critical/Important/Minor format:

**Critical:** (architectural change with no Claude.md, Requirements/Boundaries contradicted by changes)
**Important:** (missing ledger entry for significant change, stale requirement or boundary, hierarchy placement issues)
**Minor:** (template compliance gaps, bloated documentation, pruning opportunities)

If clean: "Ledger is up to date."

DO NOT make any changes. Report findings only.
```

### UX Tester Agent: User Experience

Dispatch using `subagent_type: "ux-tester"` via the Agent tool (not Task tool). Do NOT include the diff — this agent doesn't read code. Instead, translate the diff into features/flows to test. Prompt:

```
Do a complete UX review of the following features/flows affected by the current changes:
[list features/flows derived from the diff, with URLs/routes if identifiable]

Report using Critical/Important/Minor format. If clean: "UX is clean."
```

Skip if: no dev server is running, changes are backend-only with no UI impact, or affected routes cannot be determined from the diff.

## Aggregation

After all subagents complete:

```
# Code Review

## Critical
[issues]

## Important
[issues]

## Minor
[issues]
```

If all clear: "No issues found."

## Gate

- **Critical:** Block. Do not proceed.
- **Important:** Report and ask user.
- **Minor:** Report and continue.

