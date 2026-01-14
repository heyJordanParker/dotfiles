---
name: review
description: Use for code review - runs all reviewers in parallel on uncommitted changes
---

# Review

Full code review gate. Runs 8 parallel subagents on `git diff HEAD`.

## Instructions

Launch 8 subagents in parallel using the Task tool.

### Subagent 1: Anti-Slop

```
You are an anti-slop reviewer. Read references/anti-slop.md for guidance.

1. Run `git diff HEAD`
2. Scan all slop categories
3. Report:

**Critical:** (security holes, silent failures, hallucinated deps)
**Important:** (type escapes, YAGNI, duplication, test slop)
**Minor:** (comment slop, style, dead code)

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
2. Review for naming issues
3. Report:

**Critical:** (naming that causes confusion or bugs)
**Important:** (naming that hurts readability)
**Minor:** (naming suggestions)

If clean: "No naming issues found."
```

### Subagent 4: Simplicity

```
You are a simplicity reviewer. Read references/simplicity.md for guidance.

1. Run `git diff HEAD`
2. Check: reinvented wheels, unnecessary code, premature abstractions, complexity
3. Report:

**Critical:** (building what a library does, major over-engineering)
**Important:** (unnecessary abstractions, code not required by spec)
**Minor:** (could be simpler, inline suggestions)

If clean: "Code is appropriately simple."
```

### Subagent 5: Error Handling

```
You are an error handling reviewer. Read references/errors.md for guidance.

1. Run `git diff HEAD`
2. Hunt for: silent failures, swallowed errors, missing error handling
3. Report:

**Critical:** (silent failures that hide bugs, missing critical error handling)
**Important:** (inconsistent error handling, poor error messages)
**Minor:** (error handling improvements)

If clean: "Error handling is solid."
```

### Subagent 6: Types

```
You are a type design reviewer. Read references/types.md for guidance.

1. Run `git diff HEAD`
2. Check: type safety, type escapes, proper typing
3. Report:

**Critical:** (type unsafety that causes runtime errors)
**Important:** (type escapes, weak typing, missing types)
**Minor:** (type improvements)

If clean: "Types are well designed."
```

### Subagent 7: Tests

```
You are a test quality reviewer. Read references/tests.md for guidance.

1. Run `git diff HEAD`
2. Check: test coverage, test quality, testing mocks vs behavior
3. Report:

**Critical:** (deleted tests, security code without tests)
**Important:** (new functionality without tests, testing mocks not behavior)
**Minor:** (coverage suggestions)

If clean: "Tests are solid."
```

### Subagent 8: Verification

```
You are a verification reviewer.

1. Run `git diff HEAD`
2. Check for: untested claims, code that should be tested but isn't
3. Report:

**Critical:** (claims without evidence, security code unverified)
**Important:** (new logic without tests, error paths unverified)
**Minor:** (verification suggestions)

If clean: "Changes properly verified."
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

### Anti-Slop Categories

1. Comment slop - obvious/redundant comments
2. Over-defense - unnecessary try/catch, null checks
3. Type escapes - `any`, `as`, `!`
4. Duplication - copy-paste, similar functions
5. Style inconsistency - naming, patterns
6. Silent failures - empty catch, swallowed errors
7. Hallucinated deps - non-existent packages
8. YAGNI violations - premature abstraction

### Architecture Checks

- Dependencies flow one direction
- No bi-directional imports
- Business logic in services, not controllers
- Internals hidden (encapsulation)

### Simplicity Checks

- No reinvented wheels (use libraries)
- No code beyond spec requirements
- Abstractions have 3+ use cases
- Variables/functions with 1 use are inlined or self-documenting

### Test Quality

- Test behavior, not mocks
- No mock IDs in assertions
- New code has tests
- Error paths verified
