---
name: subagents
description: Framework for dispatching one-shot Subagents that complete a Task and return. Covers Prompting (WHAT/WHY, never HOW), the Story/Business/Goal/Verification/Process Prompt Template, Verification, Evidence, and coordinator-side ranking. For persistent teams, use /team.
---

# Subagents

One-shot Subagents complete a Task and return.
Persistent workers are Teams; use /team for iteration or multi-Slice coordination.
Every line of code is written by a Subagent during Orchestration; the coordinator preserves Context for coordination.

## 1. Prompt WHAT and WHY, never HOW

### Strip implementation detail from the Prompt
A fresh-Context Subagent finds the right solution from the Goal. Implementation detail in the Prompt biases it toward the coordinator's assumptions, does the Subagent's research for it, and breaks when the code changes. State the User pain, the capability wanted, and observable success.

Never: files, symbols, read order, library calls, "do not touch X", `src/foo.py`, `_extractMethod`, "read X first then Y", "use lizard.analyze", or "DO NOT modify __main__".

IF the Task is mechanical:
### Give exact mechanical steps
A bulk rename or format conversion is not Architectural, so specific Task steps are correct.

## 2. Scope the Subagent to its reasoning unit

### Give the full module or feature
An Architect given one method cannot judge encapsulation; a reviewer given one hunk cannot find caller regressions. The Subagent narrows itself after reading.

Example: Architect gets the module; reviewer gets the full diff plus surrounding code; `backend-engineer` gets the service boundary.

## 3. Write the Prompt Template

A dispatch Prompt has Story, Business, Goal, Verification, Architecture, and Process.
Story says what the User experiences and needs.
Business says WHY it matters, the boundaries, and the limitations.
Goal says what the Subagent delivers.
Verification says how the Subagent proves its work before returning.
Architecture orients the Subagent before the Process and marks files to change with `*`.
Process is the operating procedure the Subagent runs first and last.

### Weave findings into the section they belong to
A prior finding about User impact belongs in Story; a prior finding about a boundary belongs in Business. A trailing notes section is a dump the Subagent reads too late.

### Make Verification observable, specific, and complete
Verification is input → output, or command + expected status and body.

Example: "`npm test --grep payment` passes"; "timeout after 30s shows 'Payment timed out'".
Never: "code works", "tests added", "no errors".

Template:
  Story:
  <what the User experiences and needs>

  Business:
  <WHY it matters, boundaries, limitations>

  Goal:
  <what the Subagent delivers>

  Verification:
  <observable criteria>

  Architecture:
  <one-paragraph orientation>
  * path/to/file.ext — why it is in scope

  Process:
  1. Read every file marked * in the Architecture block
  2. One file at a time — read it, then edit it. No bulk-rewrite scripts, no shortcuts
  3. Implement against the Goal
  4. For EACH Verification item: run Verification, paste the output
  5. If a Verification item fails → fix and re-verify (loop step 4)
  6. Write your Evidence to docs/agents/<NNN>-<task-slug>/ — report.md plus screenshots
  7. Post a completion summary: what changed, what was verified, what was tricky, where the Evidence is

## 4. Dispatch independent Subagents at once

### Use background dispatch
Independent Subagents run at the same time with `run_in_background: true`.

Template:
  Agent(subagent_type: "backend-engineer", name: "worker",
        prompt: "<Story/Business/Goal/Verification + Architecture + Process>",
        run_in_background: true)

## 5. Verify, hold the Goal, and orchestrate

### Preserve Context for coordination
Do not use Edit, Write, or NotebookEdit while coordinating. Reading a wide diff yourself burns Context: you lose Orchestration Context and miss things.

### Verify small claims directly and dispatch larger Verification
Verify a single claim under 200 lines of code yourself with /trace or a direct read. Anything larger is a hard gate for a Verification Subagent.

### Restore by hand
Destructive-git restore is banned because Subagents nuke without checking and destroy real work. The Hooks enforce this.

Never: `git reset`, `git restore`, `git checkout --`, or a script.

### Take the long hard way
Repetitive work is still read then edit, file by file. A bulk-rewrite script or clever one-liner that skirts the work and hides errors is AI Slop.

## 6. Treat Subagent output as a claim until proved

### Repo behavior outranks the summary
The Subagent summary describes what it believes it did. The repo, common sense grounded in User, Architecture, and Business, and the Architect's reported outcome outrank the summary.

### Re-dispatch contradictions deeper
When a finding contradicts what the Architect reported, the finding is incomplete. Re-dispatch deeper until a Subagent reproduces the reported outcome.

### Reject effort arguments
"Done", "tests pass", and "no change needed" earn belief only after a repo check. "Out of scope", "too many files", and "too slow" are effort arguments, not scope. "It was broken before" is false until a clean baseline run proves it. "I'm blocked" is usually a skipped simple step: retry, restart the server, clear the cache, re-run.

## 7. Verify behavior, not guesses

### Require concrete input and observed output
Before accepting "done", the Subagent answers in writing: what concrete input ran and what exact output came back, which Critical Path was exercised start to finish, what was not exercised, and what breaks if production hits it.

### Treat compile checks as necessary but incomplete
Compiling, type-checking, linting, "logic is sound", and "looks correct" prove nothing about behavior. A confidence percentage is a guess wearing a number.

Example: "POSTed `{email: a@b.co}` to `/api/v1/form-submit`; got 200; redirect to `/thanks`; happy path exercised; editor variant not exercised."
Never: "build succeeds, types check, confidence 80%."

## 8. Record Evidence

### Evidence lands on disk
Evidence goes in `docs/agents/<NNN>-<task-slug>/`: `report.md` plus screenshots beside it, with the directory numbered for stable order. Later Agents reference the Evidence instead of trusting the claim.

### Send incomplete claims back
A completion claim without Evidence on disk is incomplete. Send the Subagent back to produce it before accepting the work.

## 9. Rank returned options yourself

### Strip the recommendation and keep the facts
A Subagent saw a Slice; the coordinator holds the project, its Rules, the Architect's prior calls, and sibling code. Its recommendation is one finding, not a verdict.

### Finish the research before ranking
Re-dispatch every gap in parallel and re-run Subagents that came back thin. Stop when nothing is left to investigate, never when the batch returns. Every surviving claim comes from a read.

### Eliminate before ranking
Drop every option that breaks a standard, convention, or Rule. The coordinator does this, not the Subagent.

### Rank in your own voice
Rank survivors with /pcc, then recommend one in your own voice.

Never: "the Agent recommends", "per the research", "based on the findings X is best", "Architect recommended Option N", "following the analysis Option N".

Template:
  Close every research gap. Dispatch as much in parallel as is independent. Do not stop to ask, do not deliver half-finished work, run the loop until nothing is left to investigate. Every code claim must come from a read, not a guess. Findings only — no scope changes, no ranking, no recommendation.
