---
description: Plan a feature using structured format
---

## Rules

1. User is the architect. Decisions with impact >7 require user approval (see Claude.md).
2. Plans are about decisions. No "Or"s in plans. Pick. Score both options with how confident you are they'll work in %, then pick the higher one. If both are under 85%, continue thinking, explore other options, or ask the user accordingly.
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
4. Classify domain risk:
   - **High-risk** (security, auth, payments, external APIs, data privacy) → research externally
   - **Unfamiliar** (new framework, no codebase examples, <70% confident) → research externally
   - **Strong local patterns** (codebase has clear examples, >85% confident) → skip external
5. If external research needed: use WebSearch/WebFetch for current best practices, then continue
6. Validate spec completeness before planning:
   - [ ] Success criteria defined? (how do we know it works?)
   - [ ] Edge cases identified? (empty, null, max, concurrent)
   - [ ] Error handling clear? (what fails, how?)
   - [ ] Dependencies known? (external services, other features)
   - If any missing: ask user via AskUserQuestion before proceeding
7. Draft assumptions with confidence scores
8. If any assumption <70%, ask user before proceeding
9. Draft plan with decisions (not options)
10. Validate with `pragmatic-engineering` skill
11. Present to user for approval

## Asking Questions

Use AskUserQuestion tool. Each question must be standalone:
- Include: file path, code example, confidence scores for each option
- Propose 2-4 different approaches
- Mark recommendation: "Option A (recommended, 80%)"
- User answers without reading the plan
