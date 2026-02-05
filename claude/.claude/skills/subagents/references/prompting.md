# Prompting Subagents

How to write effective subagent prompts. The core problem: orchestrators over-explain implementation, defeating fresh context.

## The Anti-Pattern: HOW Bias

When you know HOW to solve something, you instinctively dump that into the prompt. This:
- Locks the subagent into your approach (which may be wrong)
- Wastes tokens on instructions they'd figure out by reading code
- Prevents them from finding a better solution
- Creates fragile prompts that break when code changes

## Good vs Bad Prompts

### Bad: Step-by-step instructions

```
Fix the payment timeout bug:

1. Open PaymentService.php
2. Find the processPayment method on line 142
3. Add a try-catch around the Stripe API call
4. In the catch block, check if it's a timeout exception
5. If timeout, retry up to 3 times with exponential backoff
6. After retries exhausted, throw PaymentTimeoutException
7. In PaymentController.php, catch PaymentTimeoutException
8. Return a 408 response with message "Payment timed out"
9. Write a test that mocks Stripe to throw timeout
10. Verify retry behavior
```

Problems: Assumes the solution. Dictates file structure. Specifies implementation details the subagent should discover.

### Good: WHAT and WHY

```
Story: Users on slow connections see checkout spin forever, then
nothing. No error message, no retry, order stuck in "pending."

Business: 12% of failed checkouts are timeout-related. Retry logic
exists in Stripe SDK but we're not surfacing its results to the UI.

Goal: Surface timeout errors to UI and pass through Stripe retry
results.

DoD:
- Timeout shows user-friendly error message
- Stripe retry success completes the order
- Tests cover timeout and retry paths

backend/
├── Controllers/PaymentController.php   <- handles checkout endpoint
├── Services/PaymentService.php*        <- timeout logic lives here
├── Models/Order.php                    <- order status tracking
└── tests/
    └── PaymentServiceTest.php*         <- add timeout test coverage
```

### Bad: Vague one-liner

```
Fix the payment bug.
```

Problems: No context. No way to validate. No scope boundaries.

## Weaving Context

Previous findings go into the section where they belong — not a separate "gotchas" or "notes" section.

**Bad:** Dumping findings into a "Notes" or "Gotchas" section at the end.

**Good:** JWT bug goes in Story (user impact). CORS ordering goes in Business (debugging constraint). The subagent gets context where it matters.

## DoD Guidelines

DoD is how the subagent validates its own work before returning. Make it:

- **Observable** — can be verified by running something or checking output
- **Specific** — "tests pass" not "code works"
- **Complete** — covers the actual goal, not just the happy path

### Bad DoD

```
DoD:
- Code works
- Tests added
- No errors
```

### Good DoD

```
DoD:
- `npm test -- --grep "payment"` passes
- Timeout after 30s shows "Payment timed out. Please try again."
- Successful retry completes order (status changes to "paid")
- Failed retry after 3 attempts shows "Unable to process payment"
```

