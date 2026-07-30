---
name: user-testing
description: Tests code changes by tracing real Critical Paths. Lists Critical Paths affected by uncommitted changes, spawns one Subagent per Critical Path to trace Execution and find gaps, then evaluates Architecturally. TRIGGER when the Architect says "user test", "test the Critical Paths", or "trace Critical Paths".
---

# User Testing

One Process: identify the Critical Paths touched by uncommitted changes, get Architect approval, dispatch one Subagent per Critical Path, then evaluate the returned gaps.

## 1. Load current changes

Current Changes:
!`git changes`

Full Diff:
!`git diff HEAD`

Review Current Changes and Full Diff. Read the changed files in full.

IF the diff is empty:
### Stop without dispatching

Tell the Architect there are no uncommitted changes to test.

### Prepare Subagent Context

Write the Intent as one or two sentences on WHY these changes were made, focused on business motivation rather than code. Write the Summary as one paragraph covering what changed: files, Precedents, and scope. Use /show-architecture for an annotated file tree of the changed files and their immediate Context.

## 2. Enumerate Critical Paths

List every typical Critical Path touching the changed code. Present the list to the Architect and wait for approval before dispatching; the Architect may add, remove, or modify Critical Paths.

Template:
  ```markdown
  - Name: [short Critical Path label]
  - Entry point: [URL, button, or action]
  - Steps:
    1. [User action]
    2. [User action]
  - Exit: [expected end state]
  ```

### Never skip the approval gate

Show the Critical Paths and wait before dispatching Subagents.

## 3. Dispatch Subagents

Spawn one Subagent per approved Critical Path through /subagents, all in parallel. Each Subagent works independently with no shared state.

Write each dispatch with /subagents, carrying the Intent and the Summary from step 1. What this Skill adds to that Template:

  ```markdown
  Goal: Trace [Critical Path name] step by step through the code. For each step, read the
  actual code that executes. Report gaps, missing error handling, broken state transitions,
  or paths that do not work.

  Verification: every step traced to actual code with file:line references; each code path
  followed through controller, service, and model where those layers exist; gaps listed;
  state transitions verified; edge cases identified.

  Return:

  ## [Critical Path Name]

  ### Trace
  - Step 1: [file:line] — [what happens, any gaps]

  ### Gaps
  - Critical: [Critical Path-breaking issues]
  - Important: [functional gaps]
  - Minor: [rough edges]
  ```

### Never guess at code behavior

Subagents read the actual code instead of inferring from names.

IF the Architect explicitly asks for browser testing:
### Append browser testing to each Subagent Prompt

Add that after tracing the code, the Subagent loads the /agent-browser Skill and walks this Critical Path in the actual User Interface. Add that it loads the /design Skill and evaluates the User experience at each step.

Template:
  ```markdown
  Process addition: For each step, perform the action in the browser, screenshot the result to /tmp/[feature-name]/[critical-path]-step-[N].png, evaluate whether the User Interface reflects the expected state, evaluate whether the step is clear and consistent, and report visual bugs, confusing interactions, and /design findings.

  Verification addition: Each step screenshotted and visually verified; User experience evaluated per /design Skill Principles; visual bugs and interaction issues listed separately.
  ```

### Never save browser Evidence inside the repository

Browser screenshots go in `/tmp/`.

## 4. Evaluate returned gaps

After all Subagents return, evaluate the overall implementation with /pcc, then list every gap grouped by severity.

### Severity follows User capability loss

Critical means the Critical Path is broken and the User cannot complete the action. Important means the Critical Path works but has gaps such as missing Verification, poor error handling, or state leaks. Minor means the Critical Path works but has rough edges such as User experience issues, edge cases, or inconsistencies.

Template:
  ```markdown
  ## [Feature Name] — User Testing

  ### /pcc evaluation
  [pros, cons, confidence of the overall implementation]

  ### Critical
  - [Critical Path name]: [issue] ([file:line])

  ### Important
  - [Critical Path name]: [issue] ([file:line])

  ### Minor
  - [Critical Path name]: [issue] ([file:line])
  ```

## 5. Report without modifying code

This Skill evaluates only. Report findings; do not fix them.

### Browser testing requires an explicit Architect ask

Default to code tracing only.
