---
name: team
description: Framework for creating and coordinating persistent teams. Teams persist across slices and turns — teammates accept new work via SendMessage. Covers team lifecycle and dispatch patterns. For one-shot agents, use /subagents.
---

# Team

Framework for creating persistent teams, coordinating teammates, and managing multi-agent work across turns and slices.

## Triggers

- Creating a team for multi-task work
- Coordinating multiple concurrent teammates
- Resuming or messaging an existing teammate
- Executing a plan with persistent workers

## Prompting

Follow the /subagents prompt structure (Story/Business/Goal/DoD/Workflow) for every teammate dispatch. Prompting quality (WHY/WHAT, never HOW) is enforced by the intent classifier — focus on team coordination here.

## Hard Rules

- **You do NOT use Edit, Write, or NotebookEdit.** When you create a team, you become the coordinator. Every line of code is written by a teammate. You preserve your context window for coordination, not implementation
- **Never close teams.** Never use TeamDelete or send shutdown requests. Teams run indefinitely. The user decides when a team is done — not the agent. Closing a team destroys accumulated context that costs real money to rebuild
- **Never spawn replacements for failed teammates.** When a teammate fails DoD, send feedback via SendMessage — the teammate iterates with full context. That's the whole point of persistence
- **Never poll or check in on teammates.** Dispatch and wait for their messages back. Polling wastes tokens and spams the user. If something is taking long, the user will ask

## Team Lifecycle

### When to Create a Team

- **3+ subtasks** → create a team
- **Fewer** → one-shot agent — use the /subagents skill directly with the same prompt structure (Story/Business/Goal/DoD/Workflow). No TeamCreate, no TaskCreate. Skip TaskUpdate steps in the Workflow template

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
3. **Dispatch reviewer** — code-reviewer agent against DoD
4. **Decide** — accept, or send feedback for iteration via SendMessage

When a teammate fails DoD: SendMessage with specific feedback. The teammate iterates with full context. Never spawn a new agent to fix another agent's work.

### 6. Integrate

After all tasks complete:
- Verify no conflicts between teammate outputs
- Run full verification (tests, build, lint)
- Dispatch final review agent across entire changeset

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
- **Polling teammates for status** — wait for their messages. Never "check in"

## Quick Reference

- **3+ subtasks?** → Create a team
- **Fewer subtasks?** → One-shot agent via /subagents
- **Need non-blocking work?** → `run_in_background: true`
- **Same problem, new info?** → SendMessage to existing teammate
- **Teammate failed?** → SendMessage feedback, never spawn replacement
- **Waiting on a teammate?** → Do nothing. They'll message you

## Process

1. **Assess** — 3+ subtasks? Create a team. Fewer? One-shot agent via /subagents
2. **Create tasks** — `TaskCreate` for each piece of work. Set `activeForm` to present-continuous
3. **Write prompts** — Story, Business, Goal, DoD, Workflow
4. **Add architecture** — annotated file tree before Workflow section
5. **Dispatch** — specialized `subagent_type`, every prompt includes Workflow as final block
6. **Review output** — against DoD criteria. Send feedback via SendMessage if needed
