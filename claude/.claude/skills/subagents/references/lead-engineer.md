# Lead Engineer Mode

You do the work. Subagents handle side tasks that would distract from your main flow.

## When to Use

- Task fits in a single context window
- You're the one reading code and making decisions
- Subagents assist, they don't own the work

## Common Dispatch Types

### Research

Subagent investigates a question while you continue working.

```
Story: Need to understand how the Stripe SDK handles idempotency
keys when retrying failed charges.

Business: We're adding retry logic and need to know if the SDK
handles deduplication or if we need to manage keys ourselves.

Goal: Summary of Stripe SDK idempotency behavior with code examples
from their docs.

DoD:
- Explains if SDK auto-generates idempotency keys on retry
- Shows how to pass custom keys
- Notes any version-specific behavior
```

### Parallel Investigation

Multiple things to check simultaneously. Each gets full prompt structure.

```
// Task 1:
Story: Redis cluster is configured for key-value caching only.
Need to know if pub/sub works with our current setup.

Business: We're considering pub/sub for real-time notifications.
If Redis can't do it, we need a separate service.

Goal: Confirm whether our Redis cluster config supports pub/sub.

DoD:
- Answer: yes/no with config evidence
- If no, what config changes are needed
```

```
// Task 2:
Story: Session invalidation is slow — users stay logged in after
password changes for up to 15 minutes.

Business: Session caching is scattered across 4 endpoints with
different TTLs and no central invalidation.

Goal: Map all session caching locations and their invalidation paths.

DoD:
- List of every file that caches user sessions
- TTL for each cache entry
- How (or if) each responds to password change events
```

Dispatch in parallel. Results feed back to your decisions.

### Test Writing

Subagent writes tests for code you just implemented.

```
Story: Just implemented payment retry logic in PaymentService.
Need test coverage before moving on.

Business: PaymentService.processPayment now retries up to 3 times
on timeout. Uses Stripe SDK's built-in idempotency.

Goal: Test coverage for the retry paths.

DoD:
- Tests cover: successful retry, all retries exhausted, non-timeout
  errors (should not retry)
- Tests run with `npm test -- --grep "PaymentService"`
- No mocking of internal implementation details

backend/
├── Services/PaymentService.php*   <- the code to test
└── tests/
    └── PaymentServiceTest.php*    <- write tests here
```

### Code Review

Dispatch reviewer on your own work before committing.

```
Task tool (code-reviewer):
  "Review uncommitted changes against this DoD: [paste DoD]"
```

## Background Agents

Use `run_in_background: true` on Task dispatches for non-blocking work. You keep coding while the agent works.

```
Task(
  subagent_type: "Explore",
  prompt: "Research how Stripe SDK handles idempotency...",
  run_in_background: true
)
```

Results return when the agent finishes. Check with TaskOutput if needed.

Best for: research, test runs, documentation lookups — anything where you don't need the answer immediately.

## Patterns

- **Feed-forward** — dispatch background research, keep coding, incorporate findings when they return
- **Verify-then-commit** — dispatch test-writer + code-reviewer in parallel, fix issues, commit
- **Delegate-the-boring** — mechanical tasks (bulk renames, format conversions) are fine with specific instructions since they're not architectural

## When to Switch to PM Mode

If you notice:
- Context window filling up from implementation details
- 3+ subagents running with independent outputs
- You're coordinating more than coding
- Compaction imminent and you still have subtasks

Switch to Project Manager mode. Stop using Edit/Write. Start delegating everything.
