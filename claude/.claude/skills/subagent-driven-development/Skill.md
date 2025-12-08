---
name: subagent-driven-development
description: Use when executing implementation plans with independent tasks in the current session - dispatches fresh subagent for each task with code review between tasks, enabling fast iteration with quality gates
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Subagent-Driven Development

Execute plan by dispatching fresh subagent per task, with code review after each.

**Core principle:** Fresh subagent per task + review between tasks = high quality, fast iteration

## When to Use

- Staying in this session (no context switch)
- Tasks are mostly independent
- Want continuous progress with quality gates

**Don't use:** Need to review plan first (use executing-plans), tasks tightly coupled, plan needs revision.

## The Process

### 1. Load Plan
Read plan file, create TodoWrite with all tasks.

### 2. Execute Task with Subagent
```
Task tool (general-purpose):
  "Implement Task N from [plan-file].
   Read task. Implement. Write tests. Verify. Commit. Report back."
```

### 3. Review Subagent's Work
```
Task tool (code-reviewer):
  Review what was implemented against Task N requirements.
```

### 4. Apply Review Feedback
- Fix Critical issues immediately
- Fix Important issues before next task
- Note Minor issues

### 5. Mark Complete, Next Task
Repeat 2-5 for each task.

### 6. Final Review
After all tasks: dispatch final code-reviewer for entire implementation.

### 7. Complete Development
Use finishing-a-development-branch skill.

## Example Flow

```
[Load plan, create TodoWrite]
Task 1 → [dispatch implementation subagent] → [dispatch reviewer] → Mark complete
Task 2 → [dispatch implementation subagent] → [dispatch reviewer] → Fix issues → Mark complete
...
[Final review] → Done
```

## Red Flags

**Never:**
- Skip code review between tasks
- Proceed with unfixed Critical issues
- Dispatch multiple implementation subagents in parallel (conflicts)

**If subagent fails:** Dispatch fix subagent with specific instructions. Don't fix manually.

## Integration

**Required:** writing-plans, requesting-code-review, finishing-a-development-branch, test-driven-development

**Alternative:** executing-plans (parallel session)
