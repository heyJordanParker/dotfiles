# Tests

Gate for test quality and verification. Tests prove behavior, not implementation.

**Core principle:** Untested code is a guess. Test real behavior, not mocks.

## The Gate

Before commit, scan changed code for:

### 1. Missing Tests

- **New functionality without tests** – Every new behavior needs a test
- **Changed behavior without updated tests** – Tests must reflect new behavior
- **Security code without tests** – Auth, validation, access control must be tested
- **Error paths without tests** – Happy path only = half tested

**Fix:** Write tests for the actual behavior. TDD preferred: failing test → implement → refactor.

### 2. Mock Abuse

- **Testing mocks, not behavior** – Asserting mock was called, not real outcome
- **Incomplete mocks** – Missing fields the code depends on
- **Over-mocking** – Mocking "to be safe" when real deps work fine
- **Mock IDs in assertions** – `expect(id).toBe("mock-123")`
- **Mock setup >50% of test code** – More mocking than testing

**Fix:** Test real behavior. Mock only external boundaries (network, filesystem, clock).

### 3. Test Quality

- **Happy path only** – No failure/edge case tests
- **Snapshot abuse** – Tests don't verify intent, just shape
- **Test-only methods** – Production methods only called by tests
- **Brittle assertions** – Tests break on irrelevant changes
- **Setup-heavy tests** – More setup than assertion

**Fix:** Test the contract. Assert on behavior and outcomes, not internals.

### 4. Verification Gaps

- **Untested claims** – Code "handles" errors but no test proves it
- **Unverified error paths** – Catch blocks with no test exercising them
- **New logic without coverage** – Added code path with no test reaching it
- **Deleted tests** – Removed tests without replacement or justification
- **Claims without evidence** – "This works" without a test showing it

**Fix:** Every claim needs a test. Every error path needs a test. Evidence over assertions.

## Red Flags

- Deleted tests without explanation
- Mock setup longer than actual test
- `expect(mock).toHaveBeenCalled()` as sole assertion
- New feature with zero test files
- Error handling code with no failure test
- Test file that only tests the happy path
- Methods only called from test files

## Process

1. **Get diff** – `git diff HEAD`
2. **List new/changed behavior** – What should tests cover?
3. **Check test coverage** – Does a test exist for each behavior?
4. **Check test quality** – Are tests testing behavior or mocks?
5. **Check verification** – Are all claims backed by tests?
6. **Report gaps** – List untested behaviors and weak tests
