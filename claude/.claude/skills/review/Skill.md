---
name: review
description: Use for code review - catching slop, architectural issues, requesting reviews, or handling review feedback. Parameterized by review type.
---

# Review

Code review gate for quality and correctness.

## Triggers

- Before commits/PRs
- Reviewing code changes
- Receiving review feedback
- Requesting review from subagent

## Review Types

- **anti-slop** ([ref](references/anti-slop.md)) - Before commit - catch AI slop, YAGNI, dead code
- **architecture** ([ref](references/architecture.md)) - Check SOLID, encapsulation, dependency direction
- **receiving** ([ref](references/receiving.md)) - Handling feedback - verify before implementing
- **requesting** ([ref](references/requesting.md)) - Dispatch code-reviewer subagent
- **errors** ([ref](references/errors.md)) - Deep audit of error handling (silent-failure-hunter)
- **types** ([ref](references/types.md)) - Deep audit of type design (type-design-analyzer)
- **tests** ([ref](references/tests.md)) - Deep audit of test quality (pr-test-analyzer)
- **naming** ([ref](references/naming.md)) - Audit identifier clarity and conventions

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

### Review Response Pattern

1. READ - complete feedback
2. UNDERSTAND - restate requirement
3. VERIFY - check against codebase
4. EVALUATE - technically sound?
5. RESPOND - acknowledgment or pushback
6. IMPLEMENT - one item at a time

## Process

1. **Identify review type** from context
2. **Read relevant reference** for detailed guidance
3. **Apply checklist** from reference
4. **Gate on critical issues** - no proceeding until resolved
