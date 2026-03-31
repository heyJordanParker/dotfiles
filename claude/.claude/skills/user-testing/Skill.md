---
name: user-testing
description: Tests code changes by tracing real user flows. Lists flows affected by uncommitted changes, spawns one subagent per flow to trace execution and find gaps, then evaluates architecturally. Use when user says "user test", "test the flows", "trace user flows", or after completing a feature.
---

# User Testing

## Process

## Current Changes

!`git changes`

## Full Diff

!`git diff HEAD`

### 1. Identify Changed Code

Review the "Current Changes" and "Full Diff" sections above. If the diff is empty, tell the user there are no uncommitted changes to test and stop.

Read the changed files in full. Prepare two pieces of context for subagents:
- **Intent** — 1-2 sentences on WHY these changes were made (business motivation, not code details)
- **Summary** — 1 paragraph overview of what changed (files, patterns, scope)

Use /show-architecture to build an annotated file tree of the changed files and their immediate context.

### 2. Enumerate User Flows

List every typical user flow that touches the changed code.

For each flow:
- **Name** — short label (e.g., "New user signup", "Edit billing address")
- **Entry point** — where the user starts (URL, button, action)
- **Steps** — numbered sequence of user actions
- **Exit** — expected end state

Present the flow list to the user. Wait for approval before dispatching agents. The user may add, remove, or modify flows.

### 3. Dispatch Subagents

Spawn one subagent per approved flow using the /subagents skill, all in parallel. Each subagent works independently — no shared state or cross-referencing between them.

**Prompting:**
- Tell subagents WHAT and WHY, never HOW — give them the flow steps and let them trace the code themselves
- Scope to the reasoning unit — each subagent gets the full flow, not individual steps
- Avoid bias — don't highlight specific areas of concern. Let the subagent find what matters
- Each subagent prompt uses Story/Business/Goal/DoD structure

#### Code Tracing (default)

Each subagent's prompt:

```
Story: A user is performing [flow name]. We need to verify that
recent code changes don't break this flow and that no gaps exist
in the execution path.

Business: [intent — WHY these changes were made]

What changed: [summary paragraph]

Goal: Trace [flow name] step by step through the code. For each
step, read the actual code that executes (controllers, services,
models, middleware). Report any gaps, missing error handling,
broken state transitions, or paths that don't work.

Steps to trace:
[numbered steps from flow definition]

DoD:
- Every step traced to actual code (file:line references)
- Each code path followed through controller → service → model
- Gaps listed: missing validations, unhandled states, dead code paths
- State transitions verified: does step N's output feed correctly into step N+1?
- Edge cases identified: what happens if the user does something unexpected at each step?

Report format:
## [Flow Name]

### Trace
- Step 1: [file:line] — [what happens, any gaps]
- Step 2: ...

### Gaps
- **Critical:** [flow-breaking issues]
- **Important:** [functional gaps]
- **Minor:** [rough edges]

[annotated file tree from step 1]
```

#### Browser Testing (only when user explicitly requests)

When the user asks for browser testing, add to each subagent's prompt:

```
After tracing the code, load the /agent-browser skill and walk
through this flow in the actual UI. Load the /design skill and
evaluate the UX at each step.

Save all screenshots to /tmp/[feature-name]/ — never save files
inside the repo.

For each step:
1. Perform the action in the browser
2. Screenshot the result to /tmp/[feature-name]/[flow]-step-[N].png
3. Evaluate: Does the UI reflect the expected state?
4. Evaluate UX: Is this step clear, intuitive, and consistent?
5. Report any visual bugs, confusing interactions, or design issues

Additional DoD:
- Each step screenshotted and visually verified
- UX evaluated per /design skill principles
- Visual bugs and interaction issues listed separately
```

### 4. Evaluate

After all subagents return:

1. Evaluate the overall implementation using /pcc — assess the changes as a whole with pros/cons/confidence
2. List every gap and issue found across all flows, grouped by severity:
   - **Critical** — flow is broken, user cannot complete the action
   - **Important** — flow works but has gaps (missing validation, poor error handling, state leaks)
   - **Minor** — flow works but has rough edges (UX issues, edge cases, inconsistencies)

Output format:

```
## [Feature Name] — User Testing

### /pcc evaluation
[pros/cons/confidence of the overall implementation]

### Critical
- [flow name]: [issue] ([file:line])

### Important
- [flow name]: [issue] ([file:line])

### Minor
- [flow name]: [issue] ([file:line])
```

## Boundaries

- Never modify code — this skill only evaluates. Report findings, don't fix them
- Never skip the approval gate — always show flows and wait before dispatching subagents
- Never guess at code behavior — subagents must read the actual code, not infer from names
- Never save files inside the repo — browser screenshots go in /tmp/
- Browser testing only when explicitly requested — default is code tracing only
