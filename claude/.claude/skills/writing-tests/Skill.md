---
name: writing-tests
description: MANDATORY when writing tests. Test behavior, not implementation. Functional tests are primary. Unit tests ONLY for state mutations. Includes fixing flaky tests with condition-based waiting.
---

# Writing Tests

Test that the feature works, not that code runs.

## Principles

1. **Tests that matter** - Not coverage theater. One test proving behavior > ten tests proving code ran.
2. **No defensive programming** - Let code fail. Don't catch and swallow. Failures are information.
3. **Make it deletable** - Tests simple enough to rewrite. No elaborate fixtures or Enterprise patterns.
4. **Be pragmatic** - Write the obvious tests. Skip clever ones.
   - Setup longer than test = wrong.
   - "One assertion per test" is cargo cult.
   - If you're mocking 5 things, test is worthless. Avoid mocks and test vs a live server.

## Before

**Server is truth.** Test the real backend. Mock only what crosses the boundary (HTTP request shape, not the database).

**Decide test type:**
1. **Functional** (most tests) - Full system through entry points. Mock only boundary input.
2. **Integration** (few) - Through UI with Playwright. Critical paths only.
3. **Unit** (rare) - ONLY for pure state mutations: sanitizers, calculators, state machines.

More than 5 unit tests per feature? You're testing implementation, not behavior.

**The user story is the test:**
- "When a user uploads an image, it appears as WebP on S3"
- "When a user submits the form, their order is created"
- "When the API returns an error, the user sees an error message"

## During

### Fail Fast, Fail Loud

No defensive programming. Errors are information — let them surface. No silent failures. No endless spinners.

**Backend:** Let exceptions propagate.
```php
// WRONG
try { return $this->disk->get($key); }
catch (Exception $e) { return null; }  // Silent failure

// RIGHT
return $this->disk->get($key);  // Exception propagates
```

**Controllers:** Return renderable errors.
```php
try {
    $this->service->process($request);
    return response()->json(['success' => true]);
} catch (ValidationException $e) {
    return response()->json(['error' => $e->getMessage()], 422);
}
```

### Fixing Flaky Tests

Wait for conditions, not arbitrary timeouts.

```typescript
// WRONG: Guessing at timing
await new Promise(r => setTimeout(r, 50));
const result = getResult();

// RIGHT: Waiting for condition
await waitFor(() => getResult() !== undefined);
const result = getResult();
```

| Scenario | Pattern |
|----------|---------|
| Wait for event | `waitFor(() => events.find(e => e.type === 'DONE'))` |
| Wait for state | `waitFor(() => machine.state === 'ready')` |
| Wait for count | `waitFor(() => items.length >= 5)` |

See [flaky-tests-example.ts](flaky-tests-example.ts) for waitFor implementation.

**Arbitrary timeout is correct ONLY when:**
1. Testing actual timing behavior (debounce, throttle)
2. Based on known timing, not guessing
3. Comment explains WHY

## After

**Red Flags:**
| If you see this... | Stop and reconsider |
|-------------------|---------------------|
| More than 5 unit tests per feature | Testing implementation, not behavior |
| `try/catch` returning null | Silent failure |
| Test calls method directly instead of entry point | Bypassing how feature runs |
| 50 tests pass, feature broken | Wrong tests entirely |
| Test uses local filesystem, prod uses S3 | Testing different code than production |
| Elaborate fixtures, factories, builders | Over-engineered. Make it deletable. |

**Before commit:**
- [ ] Does the test verify what the user experiences?
- [ ] If you delete the feature code, does the test fail?
- [ ] Can errors reach the user (not just logs)?
- [ ] No arbitrary timeouts (or documented why)?
