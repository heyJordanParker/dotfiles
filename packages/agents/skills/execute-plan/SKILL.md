---
name: execute-plan
description: Orchestration Process for executing Plans. Assigns Slices to a persistent team and runs Verification after each Slice. The Agent coordinates — it does not implement.
---

# Execute Plan

- The Agent coordinates a persistent team to implement a Plan Slice by Slice.
- Every line of code is written by a Subagent.
- The coordinating Agent reads summaries and spot-checks Evidence.
- Slices run sequentially because later Slices depend on earlier changes.
- The team persists after all Slices for fixes, iteration, and follow-ups.

## 1. Check Plan readiness

Read the Plan before creating the team. The Plan file must exist, be Architect-approved, include Slices with acceptance criteria, and include WHY.

IF the Plan is missing, unapproved, lacks Slices with acceptance criteria, or lacks WHY:
### Stop and tell the Architect what is missing
Do not start Execution until the missing piece exists.

The Plan's Architecture is immutable. Stop for Architect approval before changing file structure, module boundaries, dependency direction, Precedent to follow, data ownership, API contracts, or scope.

The Agent owns private variable/function names within conventions, error wording, internal implementation, test organization, and comments.

## 2. Create the persistent team

Use /team with existing specialized agents only: `backend-engineer`, `frontend-engineer`, `architect`, and `context-engineer`.

### Use existing specialized agents only
Do not create custom agents for Plan Execution.

### Keep implementation inside Subagents
The coordinating Agent does not use Edit, Write, or NotebookEdit, and does not read full implementation files.

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

## 6. Execute each Slice sequentially

For each Slice in order: `TaskCreate`, report `Starting Slice N/M: [name]`, dispatch the implementing Subagent, run the test suite against the baseline, dispatch a fresh Verification Subagent, fix and re-verify failures up to three times, stage the Slice, report, and `TaskUpdate` to completed.

### Create and close one Task per Slice
Use `TaskCreate` with present-continuous `activeForm`; use `TaskUpdate` to completed or failed. Never leave Tasks hanging.

### Keep Slices sequential
Parallel Slice Execution creates merge conflicts and ordering bugs.

### Classify scope additions before acting
Must-have blocks the Slice, so report immediately and wait for Architect approval. Nice-to-have is logged, reported in the Slice summary, and not implemented. Out-of-scope is noted in completion and not implemented.

### Use the implementing Subagent Template
Dispatch via /team; the team persists across Slices, sharing learnings. Weave prior Slice learnings into Story or Business.

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
  - If any assumption is WRONG, stop and report to the coordinating Agent

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

## 7. Verify the Slice independently

After the implementing Subagent returns, run the test suite and compare to the baseline. Then dispatch a fresh Verification Subagent, not on the team; fresh Context prevents bias from the implementing Subagent.

### Do not trust Subagent success reports
Every Slice verifies before staging, and Verification comes from a fresh Subagent that was not on the team.

Template:
  ```
  Story: Slice [N] of [Plan name] was just implemented. We need to
  verify it meets its WHY, not just its criteria, and does not
  regress existing behavior.

  Business: Agent success reports are unreliable. Independent Verification
  catches gaps that self-reported Verification misses. Criteria can be incomplete.

  Goal: Verify this Slice against its acceptance criteria, its stated
  WHY, regressions, and cross-module interactions.
  If User Interface changes were made, use /agent-browser to verify visually.

  Verification:
  - Verify each acceptance criterion across all 4 categories:
    functional, regression, dependency audit, Architecture
  - WHY check: does this Slice achieve the WHY stated in the WHY and Story, not just the listed criteria?
  - Cross-module check: what other modules interact with modified code?
    For each interaction point, does the change create a new failure path?
  - Browser test: if the Slice has User Interface changes, use /agent-browser to verify.
    If the development server is not running, start it or report the blocker; never skip
  - Gaps: anything not covered by criteria that broke or degraded

  Architecture:
  [Annotated file tree from the Plan's Changes section, with * marking files to inspect]

  Process:
  1. Read every file marked * in the Architecture block
  2. Verify against the Goal
  3. For each Verification item: run Verification and paste the output
  4. Post the report:
     ## Slice [N] Verification
     ### WHY Check — PASS/FAIL with Evidence
     ### Acceptance Criteria — per criterion PASS/FAIL with Evidence
     ### Regressions — per file PASS/FAIL
     ### Cross-Module Interactions — per module impact
     ### Browser Test (if applicable)
     ### Gaps
  ```

### Spot-check at least one PASS
Read the Evidence yourself. If the Evidence does not support the claim, re-dispatch Verification.

### Fix and re-verify failures three times
On Verification failure, dispatch a fix Subagent with the specific failures and re-verify. After three failed fix attempts, halt with what each attempt tried, why each failed, root-cause theory, and alternatives.

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

### Keep the team open
Do not close the team after final Verification. It persists for fixes, iteration, and follow-ups.
