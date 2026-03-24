---
name: subagents
description: Framework for dispatching, resuming, and managing subagents. Covers prompting (WHAT/WHY, never HOW), validation via DoD, and two operating modes.
---

# Subagents

Framework for dispatching, resuming, and managing subagents.

## Triggers

- Dispatching any subagent (implementation, research, review)
- Choosing between doing work yourself vs delegating
- Resuming a previous subagent
- Managing multiple concurrent agents

## Prompting

### Tell subagents WHAT and WHY. Never HOW.

Subagents have fresh context. Detailed implementation instructions bias them toward your assumptions instead of letting them find the right solution.

When you know HOW to solve something, you instinctively dump that into the prompt. This:
- Locks the subagent into your approach (which may be wrong)
- Wastes tokens on instructions they'd figure out by reading code
- Prevents them from finding a better solution
- Creates fragile prompts that break when code changes

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

Problems: Assumes the solution. Dictates file structure. Specifies implementation details the subagent should discover.

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

**Good:** JWT bug goes in Story (user impact). CORS ordering goes in Business (debugging constraint). The subagent gets context where it matters.

### DoD Guidelines

DoD is how the subagent validates its own work before returning. Make it:

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

## Operating Modes

Select based on task complexity and context window needs.

### Lead Engineer

**When:** Task fits in a single context window without compaction.

You read code, write code, make decisions. Subagents handle side work — research, parallel investigations, fetching inputs. Results feed back to you.

See [lead-engineer.md](references/lead-engineer.md) for patterns.

### Project Manager

**When:** Task requires multiple compactions, touches many systems, or has 3+ independent subtasks.

Create a team with persistent teammates. Coordinate via messages and shared task list. **You do NOT write code. You do NOT read implementation details. You do NOT use Edit, Write, or NotebookEdit tools.** Every line of code is written by a teammate.

See [project-manager.md](references/project-manager.md) for lifecycle and patterns.

## Prompt Structure

Every subagent dispatch uses these sections:

- **Story** — What the user experiences and needs
- **Business** — Why this matters, constraints, limitations
- **Goal** — What the subagent delivers, expected output
- **DoD** — How the subagent validates its own work
- **Workflow** — Task state transitions that frame the work (see below)

### Workflow Section

Every prompt ends with a Workflow section. This is the agent's operating procedure — not a footnote. It's the first thing the agent does and the last thing the agent does, sandwiching all implementation work.

```
Workflow:
1. TaskUpdate task #N to in_progress
2. Read every file marked * in the architecture block above
3. Implement against the Goal
4. For EACH DoD item: run verification, paste relevant output
5. If any DoD item fails → fix and re-verify (loop step 4)
6. Post a completion summary: what changed, what was verified, what was tricky
7. TaskUpdate task #N to completed
```

### Architecture Block

After the prompt, include an annotated file tree + 1 paragraph of context:

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

## Persistence

Choose based on coordination needs:

- **Teams** — PM mode default. Persistent teammates with shared task list and messaging. Survive between turns, iterate without losing context
- **Background + resume** — Lead Engineer feed-forward. `run_in_background: true` for non-blocking side work. Resume with new direction
- **Fresh spawn** — One-shot tasks. Research, reviews, validation. No state between invocations

## Quick Reference

- **Fits in one context window?** → Lead Engineer
- **3+ independent subtasks?** → Project Manager (create a team)
- **Need non-blocking side work?** → Background agent (`run_in_background: true`)
- **Same problem, new info?** → Resume agent
- **Different problem?** → Spawn new agent
- **Want to give step-by-step?** → Stop. Give WHAT/WHY instead.
- **Scoping to one method for review?** → Stop. Give the full module/feature.

## Process

1. **Select mode** — Lead Engineer or Project Manager
2. **Create tasks** — `TaskCreate` for each piece of work. Set `activeForm` to present-continuous (e.g., "Fixing payment timeout"). This gives the user real-time visual progress via spinners and checkmarks
3. **Write prompts** — Story, Business, Goal, DoD, Workflow
4. **Add architecture** — annotated file tree before Workflow section
5. **Dispatch** — every prompt includes the Workflow section as its final block
6. **Review output** — against DoD criteria
