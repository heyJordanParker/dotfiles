---
name: testing-anti-patterns
description: Use when writing or changing tests, adding mocks, or tempted to add test-only methods to production code - prevents testing mock behavior, production pollution with test-only methods, and mocking without understanding dependencies
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Testing Anti-Patterns

Tests must verify real behavior, not mock behavior.

**Core principle:** Test what the code does, not what the mocks do.

## The Iron Laws

```
1. NEVER test mock behavior
2. NEVER add test-only methods to production classes
3. NEVER mock without understanding dependencies
```

## Anti-Pattern 1: Testing Mock Behavior

Asserting on mock elements (`*-mock` test IDs) tests that the mock exists, not that the component works.

**Gate:** Before asserting on any mock element, ask: "Am I testing real behavior or just mock existence?" If mock existence → delete assertion or unmock.

## Anti-Pattern 2: Test-Only Methods in Production

Methods like `destroy()` only called in tests pollute production code with test concerns.

**Gate:** Before adding a method, ask: "Is this only used by tests?" If yes → put in test utilities instead.

## Anti-Pattern 3: Mocking Without Understanding

Over-mocking "to be safe" breaks side effects the test depends on.

**Gate:** Before mocking:
1. What side effects does the real method have?
2. Does this test depend on any of them?
3. If unsure: run with real implementation first, then mock minimally.

## Anti-Pattern 4: Incomplete Mocks

Partial mocks (only fields you think you need) fail silently when code depends on omitted fields.

**Gate:** Mock the COMPLETE data structure as it exists in reality, not just fields your test uses.

## Anti-Pattern 5: Tests as Afterthought

Implementation "complete" without tests violates TDD.

**Gate:** TDD cycle: Write failing test → Implement to pass → Refactor → THEN claim complete.

## Quick Reference

| Anti-Pattern | Fix |
|--------------|-----|
| Assert on mock elements | Test real component or unmock it |
| Test-only methods in production | Move to test utilities |
| Mock without understanding | Understand dependencies first |
| Incomplete mocks | Mirror real API completely |
| Tests as afterthought | TDD - tests first |

## Red Flags

- Assertion checks for `*-mock` test IDs
- Methods only called in test files
- Mock setup is >50% of test
- Test fails when you remove mock
- Mocking "just to be safe"

## TDD Prevents These

1. Write test first → think about what you're testing
2. Watch it fail → confirms test tests real behavior
3. Minimal implementation → no test-only methods creep in
4. Real dependencies → see what test needs before mocking

## References

- [examples.md](examples.md) - Detailed code examples for each anti-pattern
