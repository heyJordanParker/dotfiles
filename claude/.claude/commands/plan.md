---
description: Plan a feature using structured format
---

## Rules

1. User is the architect. Decisions with impact >7 require user approval (see Claude.md).
2. Plans DECIDE — don't say "X or Y". Pick one, add confidence %. If unsure, ask.
3. Every assumption and decision needs a confidence score.
4. Clarifying questions go through AskUserQuestion. Include full context IN the question so user doesn't need to read the plan.
5. When asking, propose MULTIPLE DIFFERENT options. Mark your recommendation with confidence.

## Output Format

```
# [Feature Name]

## Assumptions
- [assumption] (X%)

## Plan
1. [step]

## Architecture
path/to/
├── file.ts   # what it does
└── other.ts  # what it does

## New Names (using the naming skill)
- name — description

## Won't
- [explicit exclusion]

## Risk
- [what could go wrong]

## Questions
- None (or list unresolved)
```

## Process

1. Explore codebase to understand context
2. Question requirements (does this need to exist?)
3. Delete scope (what's the 20% that solves 80%?)
4. Draft assumptions with confidence scores
5. If any assumption <70%, ask user before proceeding
6. Draft plan with decisions (not options)
7. Validate with `pragmatic-engineering` skill
8. Present to user for approval

## Asking Questions

Use AskUserQuestion tool. Each question must be standalone:
- Include: file path, code example, confidence scores for each option
- Propose 2-4 different approaches
- Mark recommendation: "Option A (recommended, 80%)"
- User answers without reading the plan
