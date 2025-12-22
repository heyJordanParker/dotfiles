---
name: writing-tests
description: MANDATORY when writing tests. Test behavior, not implementation. Functional tests are primary. Unit tests ONLY for state mutations. Errors must surface to users, not spin silently.
---

# Writing Tests

## Core Philosophy

**Test that the feature works, not that code runs.**

Don't test that a hook is registered. Test that uploading an image actually produces a WebP on S3.

Don't test that a method returns true. Test that the user's data is actually in the database.

## Test Types (In Order of Importance)

### 1. Functional Tests (Primary - Most Tests)

Test the full system through backend entry points. Mock only the boundary input.

| Direction | Mock | Test Real |
|-----------|------|-----------|
| Backend | Frontend request shape | Full backend: hooks, storage, DB |
| Frontend | API response shape | Full frontend logic |

**The user story is the test:**
- "When a user uploads an image, it appears as WebP on S3"
- "When a user submits the form, their order is created"
- "When the API returns an error, the user sees an error message"

### 2. Integration Tests (Few - Critical Paths Only)

Through UI with Playwright. Validate that functional test assumptions match reality.

### 3. Unit Tests (Rare - State Mutations Only)

**STOP. Before writing a unit test, ask: "Is this testing a pure state mutation?"**

Unit tests are ONLY for:
- Pure functions (sanitize string → get sanitized string)
- Calculators (input numbers → output number)
- State machines (action → new state)

**Do NOT write unit tests for:**
- Testing that hooks fire
- Testing that methods call other methods
- Testing return values that aren't state
- Testing "coverage" of code paths
- Testing internal implementation details

If you're writing more than 5 unit tests for a feature, you're doing it wrong.

## Fail Fast, Fail Loud

Errors must be visible. No silent failures. No endless spinners.

### Backend: Throw Exceptions

**WRONG:**
```php
try { return $this->disk->get($key); }
catch (Exception $e) { return null; }  // User sees nothing, spinner forever
```

**RIGHT:**
```php
return $this->disk->get($key);  // Exception propagates, error visible
```

### Controllers: Return Renderable Errors

Exceptions must become responses the frontend can display.

**WRONG:**
```php
public function store(Request $request) {
    $this->service->process($request);  // Throws, returns 500, frontend spins
}
```

**RIGHT:**
```php
public function store(Request $request) {
    try {
        $this->service->process($request);
        return response()->json(['success' => true]);
    } catch (ValidationException $e) {
        return response()->json(['error' => $e->getMessage()], 422);
    }
    // Let unexpected exceptions bubble up as 500 with stack trace in dev
}
```

The frontend must know when something fails. Silence is the enemy.

## Writing a Functional Test

```php
it('creates order when user submits checkout', function() {
    // Mock: what frontend sends
    $request = ['product_id' => 1, 'quantity' => 2];

    // Real: full backend through entry point
    $response = $this->post('/api/checkout', $request);

    // Assert: actual outcome the user cares about
    expect($response->status())->toBe(200);
    expect(Order::where('product_id', 1)->exists())->toBeTrue();
    expect(Storage::disk('s3')->exists('receipts/order-1.pdf'))->toBeTrue();
});
```

## Red Flags

| If you see this... | Stop and reconsider |
|-------------------|---------------------|
| More than 5 unit tests per feature | You're testing implementation, not behavior |
| `try/catch` returning null | Silent failure waiting to happen |
| Test calls method directly instead of entry point | Bypassing how the feature actually runs |
| Asserting on return values only | Not verifying actual outcomes |
| 50 tests pass, feature broken | Wrong tests entirely. Reexamine fundamental testing methodology. |
| Test uses local filesystem, prod uses S3 | Testing different code than production |

## Before You Commit

- Does the test verify what the user experiences?
- If you delete the feature code, does the test fail?
- Can errors reach the user (not just logs)?
