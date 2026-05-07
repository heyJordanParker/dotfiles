---
name: regressions
description: Use when checking changes for capability regressions — loss of user-facing capability ("the user can no longer X") or loss of system capability ("our system can no longer Y"). Maps a diff to affected user flows and system capabilities, traces each through the code, reports any that no longer work. Triggers on "/regressions", "check for regressions", "regression review", or before merging changes that touch user flows or system functions.
---

# Regressions

A regression is loss of user-facing capability — "the user can no longer X" — or loss of system capability — "our system can no longer Y". Trace user flows and system capabilities through the diff; flag any that no longer work.

## Execution

### 1. Get the diff

Run `git diff HEAD` for uncommitted changes. If the dispatcher provides a scope, use that diff instead.

### 2. Map the diff to capabilities

For each changed area, identify:
- **User-facing flows** — auth, checkout, search, profile editing, content creation, navigation, any interaction the user performs
- **System capabilities** — background jobs, webhooks, integrations, APIs, scheduled tasks, caches, queues, message handling, data pipelines, any function the system performs

### 3. Trace each affected capability end-to-end

For each user flow: does the user still complete the flow with the same outcome? Step through the code from the entry point (route, event handler, command) to the result.

For each system capability: does the system still perform the function with the same guarantees? Step through the code from the trigger (schedule, queue, event) to the effect.

### 4. Report

Capabilities that are broken or degraded — never report internal refactors that preserve capability.

```
**Critical:** (capability lost)
- "The user can no longer [X]" — broken at file:line
- "Our system can no longer [Y]" — broken at file:line

**Important:** (capability degraded)
- "[Z] now [degraded outcome]" — degraded at file:line

**Minor:** (edge case)
- "[edge case description]" — at file:line
```

If clean: "No capability regressions found."

## Severity

- **Critical** — Capability fully lost. User cannot complete a flow they previously could. System cannot perform a function it previously could.
- **Important** — Capability degraded. Flow completes with worse outcome (slower, less reliable, partial result, missing edge case).
- **Minor** — Capability retained at the main path, edge case lost.

## Rules

- Trace actual user flows and system capabilities, not symbol changes
- Include the affected capability AND the location in the diff that breaks it
- Report findings only — never propose fixes, never write code

## Verify

- [ ] Mapped every changed area to capabilities (user flows AND system capabilities)
- [ ] Traced each affected capability end-to-end through the code
- [ ] Each finding names the affected capability AND the location in the diff
- [ ] No internal refactors flagged as regressions
