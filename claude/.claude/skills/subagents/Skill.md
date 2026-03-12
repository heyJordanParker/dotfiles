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

Create a team with persistent teammates. Coordinate via messages and shared task list. **You do NOT write code. You do NOT read implementation details. You do NOT use Edit, Write, or NotebookEdit tools.** Every line of code is written by a teammate.

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

## Process

1. **Select mode** — Lead Engineer or Project Manager
2. **Create tasks** — `TaskCreate` for each piece of work. Set `activeForm` to present-continuous (e.g., "Fixing payment timeout"). This gives the user real-time visual progress via spinners and checkmarks
3. **Write prompts** — Story, Business, Goal, DoD
4. **Add architecture** — annotated file tree
5. **Dispatch** — each subagent prompt ends with: `Mark Task #N in_progress when you start. When DoD is met, mark it completed.`
6. **Review output** — against DoD criteria
