---
description: Plan a feature using the Plan Template
---

# /plan

Create a Plan the Architect can approve before Execution.

Live repository Context:

Current changes:
!`git changes`

Branch:
!`git branch --show-current`

Recent commits:
!`git log --oneline -10`

1. Explore the codebase until the current Architecture and Precedent are clear.
2. Question whether the feature needs to exist.
3. Cut scope to the smallest complete change that solves the Goal.
4. Classify risk:
   - High risk: security, authentication, payments, external services, or data privacy.
   - Unfamiliar: new dependency, new tool, no codebase Examples, or confidence below 70%.
   - Strong local Precedent: clear codebase Examples and confidence above 85%.
5. When current outside knowledge is needed, use WebSearch or WebFetch, then continue.
6. Validate Plan completeness before writing:
   - Success criteria are defined.
   - Edge cases are identified.
   - Error handling is clear.
   - Dependencies are known.
7. If anything is missing, ask the Architect via AskUserQuestion before proceeding.
8. Draft assumptions with confidence scores.
9. If any assumption is below 70%, ask the Architect before proceeding.
10. Draft the Plan with Decisions, not options.
11. Validate with /pragmatic-engineering.
12. Present the Plan to the Architect for approval.

Template:
  # [Feature Name]

  ## Assumptions
  - [assumption] (X%)

  ## Plan
  1. [step]

  ## Architecture
  [annotated file tree, per /show-architecture]

  ## New Names (using /naming)
  - name — description

  ## Won't
  - [explicit exclusion]

  ## Risk
  - [what could go wrong]

  ## Questions
  - None, or list unresolved questions.

AskUserQuestion Template:
  Context: [self-contained Context so the Architect does not need to read the Plan]
  Current state: [file path and code Example]
  Options:
  - Option A (recommended, 80%) — [tradeoff]
  - Option B (70%) — [tradeoff]
  - Option C (60%) — [tradeoff]

### The Architect owns Architecture

Architectural Decisions require Architect approval.

### Plans contain Decisions, not options

Never write "or" in a Plan. Score the options, pick the highest-confidence one, and ask the Architect only when the real choices remain below 85% confidence.

### Confidence is part of the Plan

Every assumption and Decision carries a confidence score.

### Questions are self-contained

Every AskUserQuestion includes the full Context, relevant file path, code Example, confidence scores, and 2-4 different options.

### Mark the recommendation

Every AskUserQuestion marks the recommended option with its confidence score.
