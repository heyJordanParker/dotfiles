---
name: execute-plan
description: Orchestration Process for executing Plans. Assigns Slices to Subagents. The Orchestrator coordinates, it does not implement.
---

# Execute Plan

- The Orchestrator implements the Plan's Slices through Subagents.
- Every line of code is written by a Subagent.
- The Orchestrator judges every Slice's Evidence per /delegate.
- Slices with no dependency between them execute in parallel; dependent Slices wait for what they depend on.
- Subagents stay resumable after all Slices for fixes, iteration, and follow-ups.

## 1. Check Plan readiness

Read the Plan before creating the team. The Plan file must exist, be Architect-approved, include Slices with acceptance criteria, and include WHY.

IF the Plan is missing, unapproved, lacks Slices with acceptance criteria, or lacks WHY:
### Stop and tell the Architect what is missing
Do not start Execution until the missing piece exists.

The Plan's Architecture is immutable. Stop for Architect approval before changing file structure, module boundaries, dependency direction, Precedent to follow, data ownership, API contracts, or scope.

The Agent owns private variable/function names within conventions, error wording, internal implementation, test organization, and comments.

## 2. Dispatch through /delegate

Dispatch every Slice per /delegate.

### Use existing specialized agents only
Do not create custom agents for Plan Execution.

### Keep implementation inside Subagents
The Orchestrator does not use Edit, Write, or NotebookEdit, and does not read full implementation files.

## 3. Refresh Claude.md Context

Dispatch @context-engineer before implementation, then spot-check the Claude.md changes. Subagents make wrong assumptions when Claude.md does not capture the Plan's WHY and Rules.

Write the dispatch with /delegate. Its Goal is to read the Plan and the Shaping Prompt and update the relevant Claude.md files so a Subagent reading them understands the WHY, Rules, boundaries, and Architecture.

Verification for this dispatch:
  - Relevant Claude.md files carry WHY from the Plan
  - Rules and boundaries from the Plan are reflected
  - Architecture and Precedents are documented
  - No fabricated WHY; only what the Plan and Shaping Prompts establish
  - No pre-researched content; read the files directly

## 4. Verify readiness

Start the development server, confirm database access, and confirm required services. If the Plan adds infrastructure, verify that infrastructure before the Slice that needs it.

IF readiness fails:
### Halt and report before executing Slices
Do not proceed with broken infrastructure.

## 5. Execute the Slices

For each Slice: `TaskCreate`, report `Starting Slice N/M: [name]`, dispatch the implementing Subagent, judge the Slice's Evidence per /delegate, fix and re-verify failures up to three times, stage the Slice, report, and `TaskUpdate` to completed.

### Create and close one Task per Slice
Use `TaskCreate` with present-continuous `activeForm`; use `TaskUpdate` to completed or failed. Never leave Tasks hanging.

### Sequence only dependent Slices
Independent Slices run in parallel. A Slice waits only for the Slices it depends on, and Slices touching the same files sequence to avoid merge conflicts.

### Classify scope additions before acting
Must-have blocks the Slice, so report immediately and wait for Architect approval. Nice-to-have is logged, reported in the Slice summary, and not implemented. Out-of-scope is noted in completion and not implemented.

### Use the implementing Subagent Template
Write the dispatch with /delegate; resume the same Subagents across Slices so learnings carry. Weave prior Slice learnings into Story or Business — a library limitation goes in Business, a broken test goes in Story.

What this Plan adds to that Template:

  Goal: the Plan's Architecture is immutable — adapt tactically to what you find in
  the code, never change the Architectural approach.

  Before implementing: list every assumption the Plan makes about the code you just
  read; mark each CONFIRMED with Evidence or WRONG with what is actually true; stop
  and report to the Orchestrator if any is WRONG.

  Verification: the Slice's acceptance criteria from the Plan, verbatim, plus —
  - Report any tactical deviations made and why
  - Flag any change to User-visible behavior, error handling, or authentication behavior as BEHAVIORAL CHANGE
  - For any file deletion, rename, or moved symbol: trace all references and report the chain
  - Report Slice learnings for future Slices

  Architecture: the annotated file tree from the Plan's Changes section, `*` marking files to read.

## 6. Verify the Slice from its Evidence

Judge the Slice's Evidence per /delegate against its acceptance criteria, its stated WHY, regressions, and cross-module interactions.

### Do not trust Subagent success reports
Every Slice verifies before staging, from the Evidence on disk, never from the summary.

### Require Slice Evidence beyond the criteria
The Slice's report.md covers each acceptance criterion with input and observed output, the WHY ("does this Slice achieve it, not just the listed criteria"), cross-module interaction points touched, and browser screenshots for User Interface changes.

### Fix and re-verify failures three times
On Verification failure, resume the implementing Subagent with the specific failures and re-verify. After three failed fix attempts, follow /delegate: stop resuming and dispatch a debugger for the mechanism.

IF a failure reveals an Architectural issue:
### Follow the Plan Architecture change Rule
Stop and report to the Architect what the Plan prescribed, what the Subagent found, why it does not work, and the Subagent's Proposal. Wait for approval or alternative direction. If approved, update the Plan file so future Slices see the change.

## 7. Stage and report the Slice

Stage the Slice with `git add`; report what was done, verified, tactical deviations, behavioral changes, scope additions, and Slice learnings; then `TaskUpdate` to completed.

### Never commit during Plan Execution
The Architect initiates commits.

## 8. Verify and document

After all Slices are staged, run /verify-changes, then dispatch @context-engineer to update Claude.md with what was built and the Architectural Decisions that emerged.

Report all Slices, Verification results, scope additions, behavioral changes, and overall status.

### Keep the Subagents resumable
After /verify-changes, follow-ups and fixes resume the owning Subagents.
