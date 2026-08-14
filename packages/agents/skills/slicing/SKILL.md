---
name: slicing
description: Turns a shaped and modeled change into vertical Slices with acceptance criteria and Verification requirements. Each Slice ends in demo-able functionality. The Plan becomes an immutable contract.
---

# Slicing

- Slicing turns a shaped and modeled change into vertical Slices and self-contained Plan files.
- Slicing is the third part of `Shaping → Modeling → Slicing`.
- Input is `docs/shaping/[feature]/shaping.md`, optional `frame.md`, and `affordances.md`.
- Output is `slices.md`, one `V*-plan.md` per Slice, and symlinks into `~/.claude/plans/` for Plan mode.

## 1. Read the modeled change

### Read every source before slicing
Read `frame.md` for WHY when present, `shaping.md` for requirements, boundaries, selected shape parts, and scope, and `affordances.md` for Affordances, stores, Wires Out, and Returns To.

### Resolve open flags first
A remaining ⚠️ means Slicing is not ready. Resolve it with research or the Architect before writing Slices.

### Read referenced code before proposing file changes
Read the existing code named by the Modeling artifact before deciding create or modify paths.

## 2. Break the change into vertical Slices

### A Slice ends in demo-able functionality
A Slice groups User Interface, logic, and data into a working increment. Do not create horizontal layers.
Example: core fetch plus basic render.
Never: set up all data models.

### V1 proves the core mechanism
V1 is the smallest demo-able Slice that proves the core mechanism. Later Slices layer one more mechanism each.

### Keep Slice count under nine
Combine related mechanisms when the work would exceed nine Slices.

### Assign each Affordance where it first matters
Every Affordance belongs to the Slice where it is first needed. A wire to a later Slice is a stub until that later Slice is built.

### Size by demo coherence
Merge a Slice with only one or two Affordances and no real demo. Split a Slice with fifteen or more Affordances or multiple unrelated Critical Paths.

### State the coverage
Each Slice names the Shaping parts it implements, the Rs it satisfies, and the demo statement.
Example: `Type "dharma" and results filter live`.

## 3. Explore the codebase per Slice

### Read every file the Slice will modify
For each Slice, identify Precedents to reuse, modules to reuse, existing behavior that could regress, and exact create and modify paths.

### Record unprecedented searches
When a Slice introduces a pattern with no Precedent, record the locations searched. That search becomes the Evidence the `Precedent` field cites.

## 4. Generate acceptance criteria

### Functional criteria prove the demo
Name the end-to-end behavior, expected output, and edge, empty, and error cases.

### Regression criteria protect existing behavior
Name the existing behavior, file, and how to verify it still works. Use an existing suite when it exists; otherwise name the manual Verification.

### Dependency audit criteria prevent duplicates
State the module and method verified before use, and the locations checked for an existing capability before introducing a new one.
Example: verified `SearchService` exposes `run()` before using it; checked `admin/search` and `app/search` for existing pagination.

### Boundary criteria protect scope
Name the out-of-scope area, pattern, dependency, or public contract the Slice must not touch.

## 5. Generate Verification requirements

### Verification states HOW each criterion is proven
Name the Agent, test command, browser check, API call, or observable result for each acceptance criterion.

### Implementation does not self-validate
Dispatch independent testing Subagents through `/delegate`. Trace every code path touched, exercise real User scenarios end to end, and browser-test through tester Agent plus `/agent-browser` when User Interface is involved. Full suites and test categories run once, as the Orchestrator's end gate after every Slice lands — never per Slice or per Subagent.

### Fix tactical failures autonomously
Fix issues that do not change the Architecture. Present Architectural issues to the Architect with options.

## 6. Write `slices.md` and `V*-plan.md`

Use the Templates Reference. Then create one symlink per Plan in `~/.claude/plans/`.

### The Plan is an immutable contract
Once written, the Plan defines the Architecture. The executing Agent implements it and does not redesign it. Announce this when the Plan is done.

### Architecture is fixed
Changing file layout, module boundaries, dependency direction, Precedents, data ownership, public API contracts, or scope requires Architect approval.

### Tactical execution is flexible
The Agent owns variable names within convention, error-message wording, internal implementation inside the same complexity class, test organization, and comments.

### Deviations stop Execution
When an executing Agent finds the Plan wrong or incomplete, it stops, states the issue, proposes options with tradeoffs, waits for approval, then records the approved deviation in the Slice completion summary.

### Scope additions are not absorbed silently
Must-have additions that block the Slice demo are flagged immediately with the minimal addition proposed. Nice-to-have additions are noted for later. Out-of-scope additions are summarized and not implemented.

## 7. Verify before presenting

### Every R has a satisfying Slice
Report any R with no satisfying Slice instead of hiding the gap.

### Every Slice has the required acceptance categories
Functional, Regression, Dependency audit, and Boundary criteria are mandatory.

### Every modified file has regression coverage
If a file is created or modified, the Plan names what existing behavior could regress and how to verify it.

### Every Slice has Verification and passes the demo test
The Plan must say how each demo is proven.

### No open flags remain
No ⚠️ survives into the Plan.

### Every Architectural pick names Precedent
Each file, public API, database, module boundary, and dependency direction names the file or system it builds on, or the search proving none exists.

## 8. Present the Plan

Present the full requirements table with the Slice satisfying each R, the boundaries confirmed respected, outstanding Decisions, and any R with no satisfying Slice.

## References

- Writing `slices.md`, `V*-plan.md`, Plan-mode symlinks, per-Slice Affordance tables, and optional Mermaid Slice-scope styling → [templates.md](references/templates.md)
