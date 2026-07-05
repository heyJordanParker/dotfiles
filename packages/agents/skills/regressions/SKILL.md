---
name: regressions
description: Check a diff for capability regressions — loss of User-facing capability ("the User can no longer X") or loss of system capability ("our system can no longer Y"). Maps changed code to affected Critical Path and system capabilities, traces each through the code, and reports any that no longer work. TRIGGER on "/regressions", "check for regressions", "regression review", or before merging changes that touch Critical Path or system functions.
---

# Regressions

- A regression is loss of User-facing capability: "the User can no longer X".
- A regression is also loss of system capability: "our system can no longer Y".

## 1. Get the diff

Run `git diff HEAD` for uncommitted changes.

IF the dispatcher provides a scope:
### Use the dispatcher scope instead of `git diff HEAD`

The provided scope is the diff to review.

## 2. Map the diff to capabilities

For each changed area, identify the Critical Path it touches and the system capabilities it touches.

- Critical Path includes authentication, checkout, search, profile editing, content creation, navigation, and any interaction the User performs.
- System capabilities include background jobs, webhooks, integrations, public contracts, scheduled jobs, caches, queues, Prompt handling, data pipelines, and any function the system performs.

### Trace behavior, not symbols

A symbol change is not a regression unless it causes lost or degraded User-facing capability or system capability.

## 3. Trace each affected capability end-to-end

For each Critical Path, step through the code from the entry point to the result: route, event handler, or command to outcome. Verify whether the User still completes it with the same outcome.

For each system capability, step through the code from the trigger to the effect: schedule, queue, or event to outcome. Verify whether the system still performs it with the same guarantees.

## 4. Report findings only

Report only capabilities that are broken or degraded. Each finding names the affected capability and the diff location that breaks it.

### Do not propose fixes or write code

This Skill reports regressions. It does not change files.

### Do not flag preserved capability

An internal refactor is not a regression when the capability is preserved.

Template:
    Critical: capability lost
    - "The User can no longer [X]" — broken at file:line
    - "Our system can no longer [Y]" — broken at file:line

    Important: capability degraded
    - "[Z] now [degraded outcome]" — degraded at file:line

    Minor: main path retained, edge case lost
    - "[edge case description]" — at file:line

If clean: "No capability regressions found."

## 5. Verify the audit

Every changed area is mapped to Critical Path and system capabilities.

Each affected capability is traced end-to-end through the code.

Each finding names the capability and the diff location.

No internal refactor is flagged as a regression when capability is preserved.
