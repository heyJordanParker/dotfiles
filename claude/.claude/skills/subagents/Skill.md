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

## Core Principle

**Tell subagents WHAT and WHY. Never HOW.**

Subagents have fresh context. Detailed implementation instructions bias them toward your assumptions instead of letting them find the right solution. See [prompting.md](references/prompting.md) for examples.

## Operating Modes

Select based on task complexity and context window needs.

### Lead Engineer

**When:** Task fits in a single context window without compaction.

You read code, write code, make decisions. Subagents handle side work — research, parallel investigations, fetching inputs. Results feed back to you.

See [lead-engineer.md](references/lead-engineer.md) for patterns.

### Project Manager

**When:** Task requires multiple compactions, touches many systems, or has 3+ independent subtasks.

You coordinate. You delegate. You review. **You do NOT write code. You do NOT read implementation details. You do NOT use Edit, Write, or NotebookEdit tools.** Every line of code is written by a subagent.

See [project-manager.md](references/project-manager.md) for lifecycle and patterns.

## Prompt Structure

Every subagent dispatch uses these sections:

- **Story** — What the user experiences and needs
- **Business** — Why this matters, constraints, limitations
- **Goal** — What the subagent delivers, expected output
- **DoD** — How the subagent validates its own work

Weave previous findings into whichever section they belong — a library limitation goes in Business, a broken test goes in Story. No dedicated "gotchas" section.

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

## Resuming

Save agent IDs. Resume instead of spawning new when:
- Same problem, new information or direction
- Iterating on review feedback
- Continuing interrupted work

Spawn new when the problem is different or context is stale.

## Quick Reference

- **Fits in one context window?** → Lead Engineer
- **3+ independent subtasks?** → Project Manager
- **Same problem, new info?** → Resume agent
- **Different problem?** → Spawn new agent
- **Want to give step-by-step?** → Stop. Give WHAT/WHY instead.

## Process

1. **Select mode** — Lead Engineer or Project Manager
2. **Write prompt** — Story, Business, Goal, DoD
3. **Add architecture** — annotated file tree
4. **Dispatch** — include DoD so agent self-validates
5. **Track agent ID** — for potential resume
6. **Review output** — against DoD criteria
