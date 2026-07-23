---
name: code-first
description: Autonomous async Execution. The Architect hands off and walks away; the Agent drives the Task to done — makes every Architectural call via /trace + /pcc + ranking, executes through /subagents, verifies from each implementing Subagent's Evidence (they exercise their own flows, real browser for UI), and returns finished code plus the recorded Decisions for review. TRIGGER when the Architect says "code-first", "/code-first", "execute with /subagents", "execute directly & autonomously", "drive it and report back", "iterate until done", "I'm going to bed", "off to bed", or signals an AFK / overnight handoff. DO NOT TRIGGER when the Architect wants options before action — that is the default mode (propose via /pcc and wait).
---

# Code-First

- Invoking this Skill moves the Architect's Architecture Review to the returned Decisions instead of a Proposal up front.
- Outside this Skill, every Architectural change waits for approval.
- One Process: drive the Task to done while preserving the baseline, using Subagents for Execution, and returning finished code plus the recorded Decisions.

## 1. Establish the baseline

Run the test suite before changing anything. Read the affected area until you know which User capabilities currently work. Write the baseline in Context.

### Treat the baseline as the contract

The final state matches or exceeds the entry test result and preserves every current User capability.

Example: when the Architect says "the code works right now, the tests work, that's what we expect at the end," a Subagent's broken-before claim is checked against the baseline instead of accepted.

## 2. Build the Verification plan from code

Dispatch a research Subagent with /trace and /subagents to study the affected area and map every Critical Path the Task touches: entry, input, expected output, and end state. Keep the plan fresh in Context with /loop as the work expands.

### Exercise User behavior, not confidence

Compile, type checks, and confidence numbers are not Verification. A browser-visible Critical Path goes to a Subagent carrying agent-browser, which exercises it in the real browser itself.

## 3. Make and record each Architectural call

For every Architectural call: trace the code, run /pcc, rank options against User, Architecture, and WHY, pick the best option, then record the Decision.

### Rank by correctness

Never rank an Architectural option by effort, file count, or speed.

Template:
  ```markdown
  - Decision: [what was chosen]
  - Trigger: [what surfaced the Decision]
  - Why: [User, Architecture, and WHY reason]
  - Alternatives: [options weighed and rejected]
  - Touched: [modules, contracts, or data in Architect language]
  - Verification: [command output or browser Evidence]
  - Confidence: [percent and remaining risk]
  ```

## 4. Execute through Subagents

Use /subagents for Execution. You are the Orchestrator: hold the Goal and judge each Subagent's Evidence per /subagents.

### Keep issues assigned to the Subagent that owns them

A Subagent claim is not accepted until proved. Contradictions with the Architect's reported outcomes follow /subagents contradiction handling.

### Restore manually after destructive git damage

The Agent that broke a file restores it manually; do not hide the damage behind a broad restore.

## 5. Handle blockers without stopping the run

Use /subagents claimed-blocker handling. A real blocker is recorded under Pending Architect input with the item, the paths attempted, and which boundary each path crosses. Pause that item and continue other work.

IF shared state is corrupted and needs manual restore:
### Stop the run

Do not continue on corrupted shared state.

## 6. Check long-running work on cadence

Check long-running tests, builds, and Subagents on a 30-minute wall-clock cadence. The Harness re-invokes you when tracked work finishes.

### State the wave at each check
One line per cadence: which Subagents run in parallel, which wait, and why the waiting ones cannot start. Serial work must be visible, never discovered.

### Preserve Context for synthesis

Fast polling drains the Context needed for the final Review.

IF a blocker survives two consecutive cadence checks unchanged:
### Notify the Architect instead of holding silently

Send a PushNotification naming the blocker and what it blocks, then continue other work.

## 7. Verify every iteration

Run the Verification plan every iteration from the implementing Subagents' Evidence per /subagents. Keep the baseline tests green.

### Done means the Critical Paths are green

No coping is accepted. Iterate until every Critical Path in the plan is verified.

### Validate the whole changeset once at the end

When every Critical Path is green from Evidence, dispatch code-reviewer, architect, regression-reviewer, and tester across the whole changeset. Critical blocks; findings route to the owning implementing Subagent by name. Once — never per iteration.

## 8. Return finished work and Decisions

Return finished code plus the recorded Decisions. Surface pending Architect input separately. The Decisions list contains Architectural calls only, not a process recap, ordered so a Decision that establishes a fact comes before Decisions that rely on it.

### Decisions are independently reviewable

The Architect accepts or rejects each Decision independently. A rejected Decision re-dispatches that Slice with the correction; accepted Decisions stand.
