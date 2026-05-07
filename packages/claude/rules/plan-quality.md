---
paths: **/.claude/plans/*.md
---

# Plan Quality

Plans are decisions, not discussions. Validate before writing.

## Forbidden Patterns

**Hedging** — validate first, or state "Unknown - need to verify X"
- "might be", "probably", "should be", "likely", "I think", "I believe", "perhaps", "could be"

**Unresolved decisions** — use AskUserQuestion BEFORE writing plan
- "should we", "shall we", "do we want", "question:", "TBD", "TODO", "to be determined"

**Multiple options** — pick one, plan is the chosen path
- "Option 1:", "Option 2:", "Approach A:", "Alternatively,", "we could either"

## Required Sections

- `## Definition of Done` — checklist of acceptance criteria
- `## Verification` — how to test changes work

## Architecture Over Tactics

- Describe intent, reference file paths
- Code blocks ≤10 lines (snippets, not implementations)
- Focus: why, requirements, constraints, DOD
- Avoid: full implementations, line-by-line instructions

## Before Writing Plan

1. All guesses validated by reading code
2. All decisions made via AskUserQuestion
3. Single chosen approach, no alternatives listed
4. DOD and Verification sections drafted
