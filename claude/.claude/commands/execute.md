---
description: Execute implementation plans with quality gates
argument-hint: [batched|subagent|parallel]
---

Use the `execute` skill to run implementation.

Execution mode: $ARGUMENTS

If no mode specified, ask which mode to use.

Modes:
- **batched** - Execute in batches, pause for review between
- **subagent** - Fresh subagent per task, review between
- **parallel** - 3+ independent failures, concurrent investigation
