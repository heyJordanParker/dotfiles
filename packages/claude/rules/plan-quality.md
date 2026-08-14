---
paths:
  - "**/docs/plans/*.md"
  - "**/docs/shaping/*/V*-plan.md"
---

### Treat Plans as Decisions
Plans are Decisions, not discussions. Validate before writing.

### Validate instead of hedging
Validate first, or state "Unknown - need to verify X".
Never: "might be", "probably", "should be", "likely", "I think", "I believe", "perhaps", or "could be".

### Resolve decisions before writing the Plan
Use AskUserQuestion before writing a Plan with unresolved decisions.
Never: "should we", "shall we", "do we want", "question:", "TBD", "TODO", or "to be determined".

### Pick one path
A Plan is the chosen path, not multiple options.
Never: "Option 1:", "Option 2:", "Approach A:", "Alternatively,", or "we could either".

### Include `## Definition of Done`
Include a `## Definition of Done` section with a checklist of acceptance criteria.

### Include `## Verification`
Include a `## Verification` section explaining how to test that changes work.

### Describe Architecture intent with file paths
Describe intent and reference file paths.

### Keep code blocks short
Code blocks are snippets, not implementations. Keep them to ten lines or fewer.

### Focus on Plan-level content
Focus on WHY, Rules, constraints, Definition of Done, and Verification.
Never: full implementations or line-by-line instructions.

IF writing a Plan:
### Validate every guess by reading code
Validate every guess by reading code before writing.
