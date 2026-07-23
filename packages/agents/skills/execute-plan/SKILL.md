---
name: execute-plan
description: Orchestration Process for executing Plans. Assigns Slices to Subagents and runs Verification after each Slice. The Orchestrator coordinates — it does not implement.
---

# Execute Plan

- The Orchestrator implements the Plan's Slices through Subagents.
- Every line of code is written by a Subagent.
- The Orchestrator judges every Slice's Evidence per /subagents.
- Slices with no dependency between them execute in parallel; dependent Slices wait for what they depend on.
- Subagents stay resumable after all Slices for fixes, iteration, and follow-ups.

## 1. Check Plan readiness

Read the Plan before creating the team. The Plan file must exist, be Architect-approved, include Slices with acceptance criteria, and include WHY.

IF the Plan is missing, unapproved, lacks Slices with acceptance criteria, or lacks WHY:
### Stop and tell the Architect what is missing
Do not start Execution until the missing piece exists.

The Plan's Architecture is immutable. Stop for Architect approval before changing file structure, module boundaries, dependency direction, Precedent to follow, data ownership, API contracts, or scope.

The Agent owns private variable/function names within conventions, error wording, internal implementation, test organization, and comments.

## 2. Dispatch through /subagents

Dispatch every Slice per /subagents.

### Use existing specialized agents only
Do not create custom agents for Plan Execution.

### Keep implementation inside Subagents
The Orchestrator does not use Edit, Write, or NotebookEdit, and does not read full implementation files.

## 3. Establish the baseline

Run the test suite and record pre-existing failures so Slice Verification distinguishes new regressions from known failures.

IF the test command is unknown:
### Ask once for the test command
After the Architect answers, use that command for the baseline and every Slice comparison.

## 4. Refresh Claude.md Context

Dispatch `context-engineer` before implementation, then spot-check the Claude.md changes. Subagents make wrong assumptions when Claude.md does not capture the Plan's WHY and Rules; the retro found this in 31 of 182 failures.

Template:
  ```
  Story: We are about to execute a Plan. Claude.md files need the Plan's WHY,
  Rules, Architecture, and boundaries before Subagents start working.

  Business: Subagents read Claude.md files for Context. When these files do not
  carry the Plan's WHY and Rules, Subagents make wrong assumptions; the retro
  found this in 31 of 182 failures.

  Goal: Read the Plan file at [path] and the Shaping Prompt at [path].
  Update the relevant Claude.md files so a Subagent reading them understands
  the WHY, Rules, boundaries, and Architecture for this work.

  Verification:
  - Relevant Claude.md files carry WHY from the Plan
  - Rules and boundaries from the Plan are reflected
  - Architecture and Precedents are documented
  - No fabricated WHY; only what the Plan and Shaping Prompts establish
  - No pre-researched content; read the files directly

  Architecture:
  [Annotated file tree of Claude.md files relevant to this Plan's scope]
  ```

## 5. Verify readiness

Start the development server, confirm database access, and confirm required services. If the Plan adds infrastructure, verify that infrastructure before the Slice that needs it.

IF readiness fails:
### Halt and report before executing Slices
Do not proceed with broken infrastructure.

## 6. Execute the Slices

For each Slice: `TaskCreate`, report `Starting Slice N/M: [name]`, dispatch the implementing Subagent, run the test suite against the baseline, judge the Slice's Evidence per /subagents, fix and re-verify failures up to three times, stage the Slice, report, and `TaskUpdate` to completed.

### Create and close one Task per Slice
Use `TaskCreate` with present-continuous `activeForm`; use `TaskUpdate` to completed or failed. Never leave Tasks hanging.

### Sequence only dependent Slices
Independent Slices run in parallel. A Slice waits only for the Slices it depends on, and Slices touching the same files sequence to avoid merge conflicts.

### Classify scope additions before acting
Must-have blocks the Slice, so report immediately and wait for Architect approval. Nice-to-have is logged, reported in the Slice summary, and not implemented. Out-of-scope is noted in completion and not implemented.

### Use the implementing Subagent Template
Dispatch per /subagents; resume the same Subagents across Slices so learnings carry. Weave prior Slice learnings into Story or Business.

Template:
  ```
  Story: [What the User will experience when this Slice is done — from
  the Plan's Slice description and demo line]

  Business: [WHY from the Plan. What problem this solves. What Rules apply.]

  [Weave in Slice learnings from previous Slices — a library limitation
  goes in Business, a broken test goes in Story]

  Goal: Implement [Slice name] as defined in the Plan. Read all files
  marked * in the Changes section. The Plan's Architecture is immutable
  — adapt tactically to what you find in the code, but do not change the
  Architectural approach.

  Before implementing:
  - List every assumption the Plan makes about the code you just read
  - For each: CONFIRMED with Evidence, or WRONG with what is actually true
  - If any assumption is WRONG, stop and report to the Orchestrator

  Verification:
  [Paste the Slice's acceptance criteria from the Plan, verbatim]
  - All acceptance criteria verified with Evidence, using command output instead of assertions
  - Report any tactical deviations made and why
  - Flag any change to User-visible behavior, error handling, or authentication behavior as BEHAVIORAL CHANGE
  - For any file deletion, rename, or moved symbol: trace all references and report the chain
  - Report Slice learnings for future Slices

  Architecture:
  [Annotated file tree from the Plan's Changes section, with * marking files to read]

  Process:
  1. Read every file marked * in the Architecture block
  2. Produce the assumption audit; stop if any assumption is WRONG
  3. Implement against the Goal
  4. For each Verification item: run Verification and paste the output
  5. If a Verification item fails, fix and re-verify by repeating step 4
  6. Post a completion summary: what changed, what was verified, what was tricky
  ```

## 7. Verify the Slice from its Evidence

After the implementing Subagent returns, run the test suite and compare to the baseline. Then judge the Slice's Evidence per /subagents against its acceptance criteria, its stated WHY, regressions, and cross-module interactions.

### Do not trust Subagent success reports
Every Slice verifies before staging, from the Evidence on disk, never from the summary.

### Require Slice Evidence beyond the criteria
The Slice's report.md covers each acceptance criterion with input and observed output, the WHY ("does this Slice achieve it, not just the listed criteria"), cross-module interaction points touched, and browser screenshots for User Interface changes.

### Fix and re-verify failures three times
On Verification failure, send the specific failures back to the implementing teammate by name and re-verify. After three failed fix attempts, halt with what each attempt tried, why each failed, root-cause theory, and alternatives.

IF a failure reveals an Architectural issue:
### Follow the Plan Architecture change Rule
Stop and report to the Architect what the Plan prescribed, what the Subagent found, why it does not work, and the Subagent's Proposal. Wait for approval or alternative direction. If approved, update the Plan file so future Slices see the change.

## 8. Stage and report the Slice

Stage the Slice with `git add`; report what was done, verified, tactical deviations, behavioral changes, scope additions, and Slice learnings; then `TaskUpdate` to completed.

### Never commit during Plan Execution
The Architect initiates commits.

## 9. Run final Verification

After all Slices are staged, run full /review against `git diff HEAD`, run full /user-testing for affected User-facing behavior, and dispatch `context-engineer` to update Claude.md with what was built and any Architectural Decisions that emerged.

Report all Slices, Verification results, scope additions, behavioral changes, and overall status.

### Keep the Subagents resumable
After final Verification, follow-ups and fixes resume the owning Subagents by name.
