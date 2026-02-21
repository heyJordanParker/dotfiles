---
name: review
description: Use for code review - runs all reviewers in parallel on uncommitted changes
---

# Review

Full code review gate. Runs 7 parallel subagents on `git diff HEAD`.

## Instructions

Launch 7 subagents in parallel using the Task tool.

### Subagent 1: Anti-Slop

```
You are an anti-slop reviewer. Read references/anti-slop.md for guidance.

1. Run `git diff HEAD`
2. Scan all 12 slop categories
3. Report:

**Critical:** (security holes, silent failures, hallucinated deps)
**Important:** (type escapes, duplication, dead code)
**Minor:** (comment slop, style, outdated patterns)

If clean: "No slop found."
```

### Subagent 2: Architecture

```
You are an architecture reviewer. Read references/architecture.md for guidance.

1. Run `git diff HEAD`
2. Check: SOLID violations, encapsulation breaks, dependency direction
3. Report:

**Critical:** (circular dependencies, major SOLID violations)
**Important:** (encapsulation breaks, wrong dependency direction)
**Minor:** (separation of concerns suggestions)

If clean: "No architecture issues found."
```

### Subagent 3: Naming

```
You are a naming reviewer. Read references/naming.md for guidance.

1. Run `git diff HEAD`
2. Check: misleading names, generic names, convention violations, abbreviation abuse
3. Report:

**Critical:** (naming that causes confusion or bugs)
**Important:** (naming that hurts readability)
**Minor:** (naming suggestions)

If clean: "No naming issues found."
```

### Subagent 4: Simplicity & Elegance

```
You are a simplicity & elegance reviewer. Read references/elegance.md for guidance.

1. Run `git diff HEAD`
2. Check: reinvented wheels, library leverage, YAGNI, complexity creep, approach quality
3. Report:

**Critical:** (building what a library does, major over-engineering)
**Important:** (unnecessary abstractions, code not required by spec)
**Minor:** (could be simpler, inline suggestions)

If clean: "Code is appropriately elegant."
```

### Subagent 5: Tests

```
You are a test reviewer. Read references/tests.md for guidance.

1. Run `git diff HEAD`
2. Check: missing tests, mock abuse, test quality, verification gaps
3. Report:

**Critical:** (deleted tests, security code without tests, untested claims)
**Important:** (new functionality without tests, testing mocks not behavior)
**Minor:** (coverage suggestions)

If clean: "Tests are solid."
```

### Subagent 6: Regressions

```
You are a regression reviewer. Read references/regressions.md for guidance.

1. Run `git diff HEAD`
2. For each changed function/export/type:
   a. Find all callers/references
   b. Read pre-change code with `git show HEAD:<path>`
   c. Verify callers still work with the new interface
3. Report:

**Critical:** (broken callers, deleted exports still referenced)
**Important:** (changed contracts, modified defaults)
**Minor:** (signature changes with few callers)

If clean: "No regressions found."
```

### Subagent 7: Ledger

```
You are a documentation ledger reviewer.

1. Run `git diff HEAD` to identify changed files
2. For each changed directory, find the nearest Claude.md file
3. Check if the changes include architectural decisions (impact 6+):
   - New patterns introduced
   - Dependencies added/removed
   - Structural changes (new files, moved code, changed interfaces)
   - Configuration changes
4. For each Claude.md near changed files, check:
   - Does a Ledger section exist?
   - Are Requirements or Boundaries affected by these changes?
   - Should a new ledger entry be added?
5. Report:

**Critical:** (architectural change with no Claude.md, Requirements/Boundaries contradicted by changes)
**Important:** (missing ledger entry for significant change, stale requirement or boundary)
**Minor:** (Claude.md exists but missing template sections)

If clean: "Ledger is up to date."
```

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

## Quick Reference

### Anti-Slop (12 categories)

1. Comment slop – obvious/redundant comments
2. Over-defense – unnecessary try/catch, null checks
3. Type escapes & design – 'any', 'as', '!', weak invariants, impossible states
4. Duplication – copy-paste, similar functions
5. Style inconsistency – naming, patterns
6. Silent failures – empty catch, swallowed errors, `?? fallback`
7. Hallucinated deps – non-existent packages
8. Outdated patterns – deprecated APIs
9. Missing edge cases – boundary conditions
10. Code complexity – nested conditionals, deep nesting
11. Security holes – hardcoded secrets, injection
12. Dead code – unused vars, commented code, re-exports

### Architecture

- Dependencies flow one direction
- No bi-directional imports
- Business logic in services, not controllers
- Internals hidden (encapsulation)

### Simplicity & Elegance

- No reinvented wheels (use libraries)
- Leverage existing deps fully
- YAGNI – no unnecessary abstractions, files, methods
- Every line maintained is a cost
- Right approach for the problem

### Tests

- Test behavior, not mocks
- New code has tests
- Error paths verified
- Every claim backed by evidence

### Regressions

- Changed signatures → callers updated
- Deleted exports → no remaining references
- Modified contracts → callers handle new behavior
- Changed defaults → existing callers still work
