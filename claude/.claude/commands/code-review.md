---
description: Run comprehensive code review with 6 parallel subagents
---

Review all uncommitted changes using 6 specialized reviewers in parallel.

## Instructions

Launch these 6 subagents in parallel using the Task tool. Each subagent should:
1. If in a git repo, run `git diff HEAD` to get uncommitted changes. Otherwise, review all files in current folder.
2. Use its assigned skill to review the code
3. Report findings in severity buckets: Critical / Important / Minor

### Subagent 1: Naming Review

```
You are a naming reviewer. Use the `naming` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the naming skill
3. Review all code for naming issues
4. Report findings:

**Critical:** (naming that causes confusion or bugs)
**Important:** (naming that hurts readability)
**Minor:** (naming suggestions)

If no issues: "No naming issues found."
```

### Subagent 2: Anti-Slop Review

```
You are an anti-slop reviewer. Use the `anti-slop` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the anti-slop skill
3. Review all code for slop patterns
4. Report findings:

**Critical:** (security holes, silent failures)
**Important:** (YAGNI violations, type escapes, duplication)
**Minor:** (comment slop, style inconsistency)

If no issues: "No slop found."
```

### Subagent 3: Documentation Review

```
You are a documentation reviewer. Use the `updating-claude-documentation` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the updating-claude-documentation skill
3. Check if changes warrant Claude.md updates:
   - New patterns introduced
   - Architectural decisions made
   - Conventions changed
4. Report findings:

**Critical:** (contradicts existing docs)
**Important:** (should document this decision/pattern)
**Minor:** (could optionally document)

If no issues: "No documentation updates needed."
```

### Subagent 4: Architecture Review

```
You are an architecture reviewer. Use the `architecture-review` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the architecture-review skill
3. Review for structural issues: SOLID violations, encapsulation breaks, dependency direction
4. Report findings:

**Critical:** (circular dependencies, major SOLID violations)
**Important:** (encapsulation breaks, wrong dependency direction)
**Minor:** (separation of concerns suggestions)

If no issues: "No architecture issues found."
```

### Subagent 5: Bug Hunting

```
You are a bug hunter. Use the `bug-hunting` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the bug-hunting skill
3. Hunt for: logic errors, security vulnerabilities, edge cases, race conditions, resource leaks
4. Report findings:

**Critical:** (security vulnerabilities, data loss risks)
**Important:** (logic errors, unhandled edge cases)
**Minor:** (potential race conditions, cleanup suggestions)

If no issues: "No bugs found."
```

### Subagent 6: Pragmatic Engineering Review

```
You are a pragmatic engineering reviewer. Use the `pragmatic-engineering` skill.

1. Get the code to review (git diff HEAD if in repo, else current folder)
2. Use the pragmatic-engineering skill
3. Review against the 5-step algorithm and red flags:
   - Are we building something that shouldn't exist?
   - Is this over-engineered? Could it be simpler?
   - Abstractions without 3+ use cases?
   - Components that take >10 minutes to rewrite?
   - Duplicating 3rd party functionality?
4. Report findings:

**Critical:** (building what shouldn't exist, major over-engineering)
**Important:** (YAGNI violations, unnecessary complexity)
**Minor:** (could be simpler, convention suggestions)

If no issues: "Code follows pragmatic engineering principles."
```

## After All Subagents Complete

Aggregate results into a unified report:

```
# Code Review Results

## Critical (must fix)
[List all critical issues from all reviewers]

## Important (should fix)
[List all important issues from all reviewers]

## Minor (consider)
[List all minor issues from all reviewers]

---
Reviewed by: naming, anti-slop, documentation, architecture, bug-hunting, pragmatic-engineering
```

If all reviewers report no issues: "All clear. No issues found by any reviewer."

## After Aggregation: Risk Assessment

Score each finding (1-100):
- **LOC**: 1 = <5 lines, 100 = full refactor
- **Architecture**: 1 = none, 100 = complete rewrite
- **1-Shot Confidence**: 1 = trivial, 100 = high risk of breaking things
- **Testability**: 1 = full automated coverage, 100 = fully manual
- **Total Risk**: Average of all scores

### Triage

**<60 Risk:** Batch fixes together, validate with user, execute.

**≥60 Risk:** Launch subagents to validate assumptions, ask detailed questions, plan carefully.

### User Validation

Every question must be **standalone**:
- Include: file path, line numbers, full before/after code, risk score, why it matters
- User should NEVER need to scroll up or reference earlier messages

### Execution

1. Present risk table
2. Ask user to approve <60 risk batch
3. For ≥60 risk: detailed questions with subagent validation
4. Execute in stages, wait for permission between stages
