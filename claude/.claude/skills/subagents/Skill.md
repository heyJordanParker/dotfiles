---
name: subagents
description: Framework for creating persistent teams and dispatching subagents. Teams are the default for complex tasks — teammates persist, accept new work via messages, and run indefinitely. Covers prompting (WHAT/WHY, never HOW) and validation via DoD.
---

# Subagents

Framework for creating persistent teams, dispatching subagents, and coordinating multi-agent work.

## Triggers

- Dispatching any subagent (implementation, research, review)
- Managing multiple concurrent agents
- Resuming a previous subagent
- Coordinating a team of teammates

## Prompting

### Tell subagents WHAT and WHY. Never HOW.

Subagents have fresh context. Detailed implementation instructions bias them toward your assumptions instead of letting them find the right solution.

When you know HOW to solve something, you instinctively dump that into the prompt. This:
- Locks the subagent into your approach (which may be wrong)
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

- **You do NOT use Edit, Write, or NotebookEdit.** When you create a team, you become the coordinator. Every line of code is written by a teammate. You preserve your context window for coordination, not implementation
- **Never close teams.** Never use TeamDelete or send shutdown requests. Teams run indefinitely. The user decides when a team is done — not the agent. Closing a team destroys accumulated context that costs real money to rebuild
- **Never spawn replacements for failed teammates.** When a teammate fails DoD, send feedback via SendMessage — the teammate iterates with full context. That's the whole point of persistence

## Team Lifecycle

### When to Create a Team

- **3+ subtasks** → create a team
- **Fewer** → one-shot agent — use the Agent tool directly with the same prompt structure (Story/Business/Goal/DoD/Workflow). No TeamCreate, no TaskCreate. Skip TaskUpdate steps in the Workflow template

### 1. Create Team

```
TeamCreate(team_name: "feature-name", description: "What we're building")
```

You may use: Read, Glob, Grep, Bash (read-only), AskUserQuestion, TeamCreate, SendMessage, TaskCreate/Update/List/Get.

### 2. Decompose

Break work into independent tasks with `TaskCreate`. Set `activeForm` to present-continuous (e.g., "Fixing payment timeout") — this drives the user's progress spinner. Each task completable by a teammate with no knowledge of other tasks.

### 3. Spawn Teammates

Use specialized `subagent_type` matched to the task domain — code-reviewer, architect, backend-engineer, frontend-engineer, researcher, tester, etc. Not general-purpose.

2-4 active teammates max. Reuse existing teammates via SendMessage for new work rather than spawning more.

```
Agent(
  subagent_type: "backend-engineer",
  team_name: "feature-name",
  name: "worker-name",
  prompt: "Story, Business, Goal, DoD + Architecture + Workflow"
)
```

Teammates persist between turns — send messages, assign new tasks, iterate on feedback without losing context.

Use `run_in_background: true` for non-blocking dispatch when you don't need results immediately.

### 4. Coordinate

- Teammates message you via SendMessage when they complete tasks or hit blockers
- Messages deliver automatically — no polling needed
- Respond via SendMessage to provide direction
- Track progress via TaskList
- **Idle is normal** — teammates go idle between turns. SendMessage wakes them. Don't spawn replacements

### 5. Review

After each teammate returns results:

1. **Read the summary** — does it match DoD?
2. **Spot check** — read 1-2 changed files (use Read, not Edit)
3. **Dispatch reviewer** — code-reviewer subagent against DoD
4. **Decide** — accept, or send feedback for iteration via SendMessage

When a teammate fails DoD: SendMessage with specific feedback. The teammate iterates with full context. Never spawn a new agent to fix another agent's work.

### 6. Integrate

After all tasks complete:
- Verify no conflicts between teammate outputs
- Run full verification (tests, build, lint)
- Dispatch final review subagent across entire changeset

## Dispatch Patterns

### Sequential (dependent tasks)

```
Teammate A completes → review → message Teammate B → review → ...
```

Use TaskUpdate blockedBy to express dependencies. Resume teammates with new direction via SendMessage.

### Parallel (independent tasks)

```
Teammate A ─┐
Teammate B ─┼→ review all → integrate
Teammate C ─┘
```

Spawn all at once. Each works independently. Watch for file conflicts.

### Pipeline (research → implement)

```
Research teammate → you digest findings → implementation teammate
```

Research teammate messages you with findings. Weave into implementation prompt's Story/Business sections.

## Common Mistakes

- **Writing "just a small fix" yourself** — delegate it. Your context is for coordination
- **Reading full implementation files** — read summaries. Spot check selectively
- **Spawning new agents instead of messaging teammates** — teammates persist. Send them new work
- **Spawning replacements for failed agents** — SendMessage feedback instead. That's the whole value of persistence
- **Skipping review** — every implementation task gets reviewed

## Quick Reference

- **3+ subtasks?** → Create a team
- **Fewer subtasks?** → One-shot agent
- **Need non-blocking work?** → `run_in_background: true`
- **Same problem, new info?** → SendMessage to existing teammate
- **Teammate failed?** → SendMessage feedback, never spawn replacement
- **Want to give step-by-step?** → Stop. Give WHAT/WHY instead
- **Scoping to one method?** → Stop. Give the full module/feature

## Process

1. **Assess** — 3+ subtasks? Create a team. Fewer? One-shot agent
2. **Create tasks** — `TaskCreate` for each piece of work. Set `activeForm` to present-continuous
3. **Write prompts** — Story, Business, Goal, DoD, Workflow
4. **Add architecture** — annotated file tree before Workflow section
5. **Dispatch** — specialized `subagent_type`, every prompt includes Workflow as final block
6. **Review output** — against DoD criteria. Send feedback via SendMessage if needed
