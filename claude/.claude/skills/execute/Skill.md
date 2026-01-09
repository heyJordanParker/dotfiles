---
name: execute
description: Use when executing implementation plans. Modes: batched with checkpoints, subagent per task, or parallel agents for independent failures.
---

# Execute

Framework for executing implementation plans.

## Triggers

- Have a plan to execute
- Multiple independent failures to investigate
- Need structured execution with quality gates

## Execution Modes

- **batched** — [batched.md](references/batched.md) — execute in batches, pause for review
- **subagent-driven** — [subagent-driven.md](references/subagent-driven.md) — fresh subagent per task, review between
- **parallel-agents** — [parallel-agents.md](references/parallel-agents.md) — 3+ independent failures, concurrent investigation

## Quick Reference

### Batched Execution

1. Load plan, create TodoWrite
2. Execute batch (default: 3 tasks)
3. Report: what was implemented, verification output
4. Wait for feedback
5. Continue or adjust

### Subagent-Driven

1. Load plan, create TodoWrite
2. Dispatch subagent for Task N
3. Review subagent's work (use code-reviewer)
4. Fix issues
5. Repeat for next task

### Parallel Agents

**Use when:**
- 3+ test files failing with different root causes
- Multiple subsystems broken independently
- No shared state between investigations

**Don't use when:**
- Failures are related
- Agents would edit same files

**Pattern:**
1. Identify independent domains
2. Dispatch one agent per domain
3. Collect results
4. Merge fixes

## Mode Selection

- **Staying in session, want speed?** → subagent-driven
- **Want review checkpoints?** → batched
- **Multiple unrelated failures?** → parallel-agents

## Process

1. **Identify execution context**
2. **Select mode** or ask user
3. **Read relevant reference**
4. **Follow mode's process exactly**
