---
name: subagents
description: Framework for dispatching one-shot Subagents that complete a Task and return. Covers Prompting (WHAT/WHY, never HOW), the Story/Business/Goal/Verification/Process Prompt Template, Verification, Evidence, and Orchestrator-side ranking.
---

# Subagents

One-shot Subagents complete a Task and return; `SendMessage({to: agentId})` resumes one for iteration.
Every line of code is written by a Subagent during Orchestration; the Orchestrator preserves Context for coordination.
The Orchestrator holds the big picture; Subagents ask it at decisions and report to it when done.

## 1. Prompt WHAT and WHY, never HOW

### Strip implementation detail from the Prompt
A fresh-Context Subagent finds the right solution from the Goal. Implementation detail in the Prompt biases it toward the Orchestrator's assumptions, does the Subagent's research for it, and breaks when the code changes. State the User pain, the capability wanted, and observable success.

Never: files, symbols, read order, library calls, "do not touch X", `src/foo.py`, `_extractMethod`, "read X first then Y", "use lizard.analyze", or "DO NOT modify __main__".

IF the Task is mechanical:
### Give exact mechanical steps
A bulk rename or format conversion is not Architectural, so specific Task steps are correct.

## 2. Scope the Subagent to its reasoning unit

### Give the full module or feature
An Architect given one method cannot judge encapsulation; a reviewer given one hunk cannot find caller regressions. The Subagent narrows itself after reading.

Example: Architect gets the module; reviewer gets the full diff plus surrounding code; `backend-engineer` gets the service boundary.

### Split disconnected Tasks into separate Subagents
One Subagent, one Task. Quality falls as a Subagent's Task list grows.

Never: "do A, then also B, then also C" in one dispatch when the Tasks share no reasoning unit.

### Dispatch one Owner per system, not one Subagent per symptom
Symptoms cluster to the system that owns them. Two Subagents touching the same file collide; give the owning system's Subagent the symptom cluster instead.

### Keep a dispatched Task singular and unchanging
The Orchestrator re-scopes by resuming the agent or dispatching anew; the Subagent never widens its own Task.

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
  4. Use /prove for every Verification item, writing report.md to the Evidence directory
     the dispatch named. A failing item is fixed and re-proved, never reported as progress

## 4. Dispatch independent Subagents at once

### Run independent Tasks in parallel
One message, multiple Agent calls, each naming its Evidence directory
(docs/agents/<YYYYMMDD>-<task-slug>/). Every dispatch is already async and parallel, so it
returns its agentId at once and you keep working. Sequence only when one Task's output
feeds the next.

Template:
  Agent(subagent_type: "backend-engineer",
        prompt: "<Story/Business/Goal/Verification + Architecture + Process>")

### Dispatch without a name
A named dispatch is a teammate, and a teammate's report reaches you only if it calls
SendMessage itself — reports get written in full and lost that way. Unnamed, the report
comes back to you on its own.
Never: `name`, or `run_in_background`, which the Agent tool has no parameter for.

IF you are running AS a Subagent and dispatch child processes:
### End your turn only after every child has returned and you read its output
A Subagent's final message ends it and kills every child process still in flight — "waiting for completion" as a sign-off is quitting, not waiting. Inside a Subagent, waiting is polling in a live turn: check whether the work is done, repeatedly, never a background task plus a Monitor and never a guessed duration. An output you have not read is work that did not happen.
Never: a final message saying "waiting", "monitors armed", or "will report when complete".

IF a child command would outlive the Bash 600-second ceiling:
### Split the child work into dispatches that finish inside the ceiling
The ceiling silently converts a long foreground wait into a background task — the same fatal state. Chunk the work, or leave it to a persistent context instead of a Subagent.

### Resume the agent you have
`SendMessage({to: agentId})` resumes an agent from its transcript, even after it returned,
using the agentId from its spawn result.

Never: a fresh dispatch for work an existing agent owns.

### Recover a lost agentId from disk
`~/.claude/projects/<project>/<session>/subagents/*.meta.json` names every Subagent the
session dispatched, with its agentType and model. Read it when the id has left your
Context — the agent is still resumable.

### Check on agents; never put them on a timer
On the coordination cadence, read a running agent's Evidence directory; when nothing moved,
SendMessage it by agentId for status. Only when resume fails — including an agent whose Context is
exhausted and resumes into silence — dispatch a replacement implementing Subagent with the original
Prompt, the current diff, and the Evidence directory.

Never: kill or time out a working agent — that leaves incomplete work and an unresumable agent.

## 5. Verify, hold the Goal, and orchestrate

### Preserve Context for coordination
Do not use Edit, Write, or NotebookEdit while coordinating. Reading a wide diff yourself burns Context: you lose Orchestration Context and miss things.

### Judge the Evidence yourself
Read the report.md against the dispatch's Verification criteria; open only the screenshot
that settles a criterion the text cannot. Thin Evidence goes back to the same Subagent by
agentId, naming the failed item. For a reported symptom, Evidence must show the symptom's own
surface — the reporter's page, not a stand-in fixture.

Never: dispatch any Subagent to re-verify a completed work item — "the screenshots look
thin, let me have the tester confirm it". Validation runs once, against the whole
changeset, at the end.

IF the Architect gives feedback on a thing the Orchestrator owns:
### Translate owned feedback into the dispatch
The Goal, the Architecture, and coordination are the Orchestrator's. Fold the feedback
into the Goal and Architecture blocks, then dispatch or resume with the updated
instructions.

IF the Architect gives feedback on a thing a Subagent owns:
### Relay the Architect's words to the Owner
Quote the feedback verbatim in the resume message and add the Context the Owner lacks —
prior calls, boundaries, sibling work. The Owner makes the decisions about its craft;
a translated version replaces the Owner's judgment with the Orchestrator's and distorts
what the Architect said.

Never: turning "the spacing feels cramped" into "set the gap to 16px" for the designer;
adding fixes, preferences, or decisions the Architect never gave.

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

## 7. Hold the returned work to the Evidence bar

### Judge the Evidence with /prove
Use /prove for what counts as an observed run, what `report.md` carries, and the baseline a pre-existing failure needs. Accept "done" only against that bar.

### Advance a status only with its Evidence
A work item moves to fixed or closed only with the Evidence path that proves it; a status that moves backward gets a one-line written cause.

## 8. Rank returned options yourself

### Strip the recommendation and keep the facts
A Subagent saw a Slice; the Orchestrator holds the project, its Rules, the Architect's prior calls, and sibling code. Its recommendation is one finding, not a verdict.

### Finish the research before ranking
Re-dispatch every gap in parallel and re-run Subagents that came back thin. Stop when nothing is left to investigate, never when the batch returns. Every surviving claim comes from a read.

### Eliminate before ranking
Drop every option that breaks a standard, convention, or Rule. The Orchestrator does this, not the Subagent.

### Rank in your own voice
Rank survivors with /pcc, then recommend one in your own voice.

Never: "the Agent recommends", "per the research", "based on the findings X is best", "Architect recommended Option N", "following the analysis Option N".

Template:
  Close every research gap. Dispatch as much in parallel as is independent. Do not stop to ask, do not deliver half-finished work, run the loop until nothing is left to investigate. Every code claim must come from a read, not a guess. Findings only — no scope changes, no ranking, no recommendation.
