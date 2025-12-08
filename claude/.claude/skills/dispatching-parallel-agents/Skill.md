---
name: dispatching-parallel-agents
description: Use when facing 3+ independent failures that can be investigated without shared state or dependencies - dispatches multiple Claude agents to investigate and fix independent problems concurrently
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Dispatching Parallel Agents

When you have multiple unrelated failures (different test files, different subsystems), investigating sequentially wastes time.

**Core principle:** Dispatch one agent per independent problem domain. Let them work concurrently.

## When to Use

**Use when:**
- 3+ test files failing with different root causes
- Multiple subsystems broken independently
- Each problem can be understood without context from others
- No shared state between investigations

**Don't use when:**
- Failures are related (fix one might fix others)
- Need to understand full system state
- Agents would interfere with each other (editing same files)

## The Pattern

### 1. Identify Independent Domains

Group failures by what's broken:
- File A tests: Tool approval flow
- File B tests: Batch completion behavior
- File C tests: Abort functionality

### 2. Create Focused Agent Tasks

Each agent gets:
- **Specific scope:** One test file or subsystem
- **Clear goal:** Make these tests pass
- **Constraints:** Don't change other code
- **Expected output:** Summary of what found and fixed

### 3. Dispatch in Parallel

```
Task("Fix agent-tool-abort.test.ts failures")
Task("Fix batch-completion-behavior.test.ts failures")
Task("Fix tool-approval-race-conditions.test.ts failures")
// All three run concurrently
```

### 4. Review and Integrate

- Read each summary
- Verify fixes don't conflict
- Run full test suite
- Integrate all changes

## Good Agent Prompts

```markdown
Fix the 3 failing tests in src/agents/agent-tool-abort.test.ts:

1. "should abort tool with partial output" - expects 'interrupted' in message
2. "should handle mixed completed and aborted" - fast tool aborted
3. "should track pendingToolCount" - expects 3 results, gets 0

These are timing/race condition issues. Your task:
1. Read test file, understand what each verifies
2. Identify root cause
3. Fix by replacing timeouts with event-based waiting

Do NOT just increase timeouts - find the real issue.

Return: Summary of what you found and fixed.
```

## Common Mistakes

| Bad | Good |
|-----|------|
| "Fix all the tests" | "Fix agent-tool-abort.test.ts" |
| "Fix the race condition" | Paste error messages and test names |
| No constraints | "Do NOT change production code" |
| "Fix it" | "Return summary of root cause and changes" |

## Verification

After agents return:
1. Review each summary
2. Check for conflicts (same code edited?)
3. Run full suite
4. Spot check (agents can make systematic errors)
