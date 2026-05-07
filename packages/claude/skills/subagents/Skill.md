---
name: subagents
description: Framework for dispatching one-shot subagents that complete a task and return. Covers prompting (WHAT/WHY, never HOW), prompt structure (Story/Business/Goal/DoD/Workflow), and validation via DoD. For persistent teams, use /team.
---

# Subagents

Framework for dispatching one-shot subagents — agents that complete a single task and return results. For persistent teams that coordinate across multiple tasks and slices, use the /team skill.

## Triggers

- Dispatching any subagent (implementation, research, review)
- Running parallel independent agents
- One-shot validation or analysis tasks

## Prompting

### Tell agents WHAT and WHY. Never HOW.

Agents have fresh context. Detailed implementation instructions bias them toward your assumptions instead of letting them find the right solution.

When you know HOW to solve something, you instinctively dump that into the prompt. This:
- Locks the agent into your approach (which may be wrong)
- Wastes tokens on instructions they'd figure out by reading code
- Prevents them from finding a better solution
- Creates fragile prompts that break when code changes

**Exception:** Mechanical tasks (bulk renames, format conversions) are fine with specific instructions since they're not architectural.

### Scope agents to their reasoning unit, not your diff.

An architect reviewing one method can't assess encapsulation. A code reviewer scoped to one hunk can't find regressions. Give review and architecture agents the full module or feature — they narrow themselves after reading the code.

- Architect → entire module/feature, not the changed file
- Code reviewer → full diff + surrounding context, not individual hunks
- Backend engineer → the service boundary, not the changed endpoint

### Good vs Bad Prompts

**Bad: Step-by-step instructions**

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

Problems: Assumes the solution. Dictates file structure. Specifies implementation details the agent should discover.

**Good: WHAT and WHY**

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

**Bad: Vague one-liner**

```
Fix the payment bug.
```

Problems: No context. No way to validate. No scope boundaries.

**Bad: Over-scoped to the diff**

```
Review the `calculateDiscount` method in PricingService.php.
Check if the new early-return is correct.
```

Problems: Architect can't assess encapsulation or dependency direction from one method. Reviewer can't find regressions in callers.

**Good: Scoped to the reasoning unit**

```
Review PricingService — we changed the discount calculation logic.
Apply your full review protocol across the service.
```

The agent narrows itself once it reads the code. Your job is to give it enough room to find things you didn't think to look for.

### Weaving Context

Previous findings go into the section where they belong — not a separate "gotchas" or "notes" section.

**Bad:** Dumping findings into a "Notes" or "Gotchas" section at the end.

**Good:** JWT bug goes in Story (user impact). CORS ordering goes in Business (debugging constraint). The agent gets context where it matters.

### DoD Guidelines

DoD is how the agent validates its own work before returning. Make it:

- **Observable** — can be verified by running something or checking output
- **Specific** — "tests pass" not "code works"
- **Complete** — covers the actual goal, not just the happy path

**Bad DoD**

```
DoD:
- Code works
- Tests added
- No errors
```

**Good DoD**

```
DoD:
- `npm test -- --grep "payment"` passes
- Timeout after 30s shows "Payment timed out. Please try again."
- Successful retry completes order (status changes to "paid")
- Failed retry after 3 attempts shows "Unable to process payment"
```

## Prompt Structure

Every agent dispatch uses these sections:

- **Story** — What the user experiences and needs
- **Business** — Why this matters, constraints, limitations
- **Goal** — What the agent delivers, expected output
- **DoD** — How the agent validates its own work
- **Workflow** — Task state transitions that frame the work (see below)

### Workflow Section

Every prompt ends with a Workflow section. This is the agent's operating procedure — not a footnote. It's the first thing the agent does and the last thing the agent does, sandwiching all implementation work.

```
Workflow:
1. Read every file marked * in the architecture block above
2. Implement against the Goal
3. For EACH DoD item: run verification, paste relevant output
4. If any DoD item fails → fix and re-verify (loop step 3)
5. Post a completion summary: what changed, what was verified, what was tricky
```

### Architecture Block

Before the Workflow section, include an annotated file tree + 1 paragraph of context:

```
Payment timeout errors silently swallow failures. The payment service
uses BaseService patterns. Controllers return WP_REST_Response objects.

backend/
├── Controllers/PaymentController.php   <- handles checkout endpoint
├── Services/PaymentService.php*        <- timeout logic lives here
├── Models/Order.php                    <- order status tracking
└── tests/
    └── PaymentServiceTest.php*         <- add timeout test coverage
```

## Hard Rules

- **You do NOT use Edit, Write, or NotebookEdit** when coordinating multiple agents. Every line of code is written by a subagent. You preserve your context window for coordination, not implementation
- **One-shot agents return and die.** They don't persist. For work that needs iteration, feedback loops, or multi-slice coordination, use the /team skill instead

## Dispatching

### Parallel (independent tasks)

Spawn all agents at once using the Agent tool. Each works independently with `run_in_background: true`.

```
Agent(
  subagent_type: "backend-engineer",
  name: "worker-name",
  prompt: "Story, Business, Goal, DoD + Architecture + Workflow",
  run_in_background: true
)
```

### Reviewing Results

After each agent returns:

1. **Read the summary** — does it match DoD?
2. **Spot check** — read 1-2 changed files (use Read, not Edit)
3. **Decide** — accept, or dispatch a new agent with feedback

## Quick Reference

- **Need persistent workers?** → Use /team skill
- **Need non-blocking work?** → `run_in_background: true`
- **Want to give step-by-step?** → Stop. Give WHAT/WHY instead
- **Scoping to one method?** → Stop. Give the full module/feature

## Process

1. **Assess** — is this one-shot work or does it need iteration? One-shot → here. Iteration → /team
2. **Write prompts** — Story, Business, Goal, DoD, Workflow
3. **Add architecture** — annotated file tree before Workflow section
4. **Dispatch** — specialized `subagent_type`, every prompt includes Workflow as final block
5. **Review output** — against DoD criteria
