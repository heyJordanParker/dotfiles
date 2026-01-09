# Test Coverage Review

Dispatch pr-test-analyzer agent to audit test quality.

**Focus:** Coverage gaps, behavioral testing, edge cases, test value.

## When to Use

- Adding new features with tests
- Test refactors
- Before merging PRs
- When unsure if tests are sufficient

## How to Request

**1. Get test files:**
```bash
git diff --name-only HEAD~1 -- '*test*' '*spec*'
```

**2. Dispatch pr-test-analyzer:**

Use Task tool with `pr-review-toolkit:pr-test-analyzer` subagent type:

```
Review test coverage in these changes:

Files: {CHANGED_FILES}
Git range: {BASE_SHA}..{HEAD_SHA}

Analyze:
- Do tests cover the actual behavior (not just mocks)?
- Are edge cases tested?
- Are error paths tested?
- Is coverage meaningful (not just lines)?
```

**3. Act on findings:**
- Add missing behavioral tests
- Remove mock-heavy tests that don't verify real behavior
- Cover identified edge cases

## What It Catches

- **Mock-heavy tests:** Tests pass but don't verify behavior
- **Missing edge cases:** Happy path only
- **No error tests:** Failure paths untested
- **Snapshot abuse:** Tests don't verify intent
- **Setup-heavy tests:** More setup than assertion

## Integration

Use before merge when tests exist but coverage quality is uncertain.
