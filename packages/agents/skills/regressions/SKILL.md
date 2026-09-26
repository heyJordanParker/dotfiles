---
name: regressions
description: Check a diff for Capability regressions — loss of User-facing Capability ("the User can no longer X") or loss of system Capability ("our system can no longer Y"). Maps changed code to affected Critical Path and system Capabilities, traces each through the code, and reports any that no longer work. TRIGGER on "/regressions", "check for regressions", "regression review", or before merging changes that touch Critical Path or system functions.
---

# Regressions

- A regression is loss of User-facing Capability: "the User can no longer X".
- A regression is also loss of system Capability: "our system can no longer Y".

## 1. Get the diff

Run `git diff HEAD` for uncommitted changes.

IF the dispatcher provides a scope:
### Use the dispatcher scope instead of `git diff HEAD`

The provided scope is the diff to review.

## 2. Map the diff to Capabilities

For each changed area, identify the Critical Path it touches and the system Capabilities it touches.

- Critical Path includes authentication, checkout, search, profile editing, content creation, navigation, and any interaction the User performs.
- System Capabilities include background jobs, webhooks, integrations, public contracts, scheduled jobs, caches, queues, Prompt handling, data pipelines, and any function the system performs.

### Trace behavior, not symbols

A symbol change is not a regression unless it causes lost or degraded User-facing Capability or system Capability.

## 3. Trace each affected Capability end-to-end

For each Critical Path, step through the code from the entry point to the result: route, event handler, or command to outcome. Verify whether the User still completes it with the same outcome.

For each system Capability, step through the code from the trigger to the effect: schedule, queue, or event to outcome. Verify whether the system still performs it with the same guarantees.

## 4. Report findings only

Report only Capabilities that are broken or degraded. Each finding names the affected Capability and the diff location that breaks it.

### Do not propose fixes or write code

This Skill reports regressions. It does not change files.

### Do not flag preserved Capability

An internal refactor is not a regression when the Capability is preserved.

Template:
    Blocking: Capability lost
    - "The User can no longer [X]" — broken at file:line
    - "Our system can no longer [Y]" — broken at file:line

    Important: Capability degraded
    - "[Z] now [degraded outcome]" — degraded at file:line

    Polish: main path retained, edge case lost
    - "[edge case description]" — at file:line

    If clean: "No Capability regressions found."
