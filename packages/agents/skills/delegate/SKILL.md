---
name: delegate
description: Framework for dispatching one-shot Subagents that complete a Task and return. Covers Prompting (WHY -> WHAT -> HOW), the Story/Business/Goal/Verification/Process Prompt Template, and Evidence directories. TRIGGER when dispatching or resuming Subagents, when Orchestration needs the dispatch Prompt Template, or when the Architect says "/delegate this to ...". DO NOT TRIGGER for judging what a Subagent returned (that is /orchestrate), or for work the Agent does itself with no Subagents; use /build.
reload-every: 20 turns
---

# Delegate

One-shot Subagents complete a Task and return; `SendMessage({to: agentId})` resumes one for iteration.
A Subagent's Context is a fixed budget spent once: the Prompt, every file it reads, and every tool result compete for the same room. Two Tasks in one dispatch halve the room each gets.

## 1. Prompt WHY, then WHAT, then the HOW you decided

### Write WHY, then WHAT, then the HOW you have decided
The WHY primes the Agent: the User pain and the Business behind it. The WHAT names the deliverable and observable success. The HOW is what you have already decided — a call you made rides in the dispatch, a file you need read is named by path, and what you left open the Agent settles itself, finding related files on its own. A contract left implicit is a decision the Agent will invent.

### Give an Agent only what its Task consumes
Context that exists to be forbidden is Context that should be absent. Never tell an Agent about sibling Agents, parallel explorations, or files it must not read — independence comes from omission, not prohibition. Your reasoning, your caveats, and what you already ruled out are the same waste: every line is room the Subagent no longer has for the code.

Never: "a parallel exploration you must not converge with", "do not read X (another agent's output)", or "do not touch Agent B's plan file".

IF the Task is mechanical:
### Give exact mechanical steps
A bulk rename or format conversion is not Architectural, so specific Task steps are correct.

## 2. Scope the Subagent to its reasoning unit

### Split by what a Task must hold in Context to decide
Two Tasks share one dispatch only when finishing the second needs what reading for the first already put in Context. A shared topic is not a shared reasoning unit: count the files each Task must read, and no overlap means separate dispatches sent in one message. Order between them means you sequence the dispatches, never a list one Subagent walks.

Example: "add the endpoint" and "update its caller in the same service" is one dispatch — the same files. "Fix the card field" and "rewrite the confirmation email" is two, sent together, even though both are checkout.
Never: a numbered Task list in one Prompt, or executing a Task list yourself instead of dispatching it.

### Size a building Task to one verifiable change
The Verification block is the ruler: criteria on more than one surface mean more than one dispatch. Many surgical dispatches in one message beat one broad one — each builder stays fast, exact, and cheap to re-dispatch when wrong.
Never: "implement the feature", "migrate the module", or a dispatch whose Verification cannot run until a later dispatch lands.

### Dispatch one Owner per system, not one Subagent per symptom
Symptoms cluster to the system that owns them. Two live Subagents touching the same file collide; give the owning system's Subagent the whole cluster in its founding dispatch. Ownership ends when the agent returns — the next cluster founds a fresh Owner.

### Keep a dispatched Task singular and unchanging
The Orchestrator corrects by resuming and re-scopes by dispatching anew; the Subagent never widens its own Task.

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

### Require the class diagram and the file tree in the Goal
A Goal that delivers Architecture requires a class diagram, never a table. A Goal that delivers file or API changes requires an annotated file tree, never a table. Both formats are /show-me's.

### Make Verification observable, specific, and complete
Verification is input → output, or command + expected status and body.

Example: "`npm test --grep payment` passes"; "timeout after 30s shows 'Payment timed out'".
Never: "code works", "tests added", "no errors".
### Name one test, not a test file
Write the name of the test that reproduces the change. When that test does not exist yet, write the behavior it must show and let the Subagent name it. The whole file and the suite are your end gate, after every Task lands.
Never: a test file path, a suite name, or a directory as a Verification command.

IF the Task fixes a bug:
### Require a regression test that fails pre-fix
The Verification block names a new test that fails on the pre-fix code and passes after, plus one line on why the existing suite missed the bug.

Template:
  Story:
  <what the User experiences and needs>

  Business:
  <WHY it matters, boundaries, limitations>

  Goal:
  <what the Subagent delivers>

  Verification:
  <observable criteria>
  <the name of the test that reproduces this change, or the behavior it must show>
  <each consumer the change reaches, what it does today, and the expected effect>

  Architecture:
  <one-paragraph orientation>
  * path/to/file.ext — why it is in scope

  Process:
  1. Read every file marked * in the Architecture block
  2. One file at a time — read it, then edit it. No bulk-rewrite scripts, no shortcuts,
     and no git reset, git restore, or git checkout -- to undo your own work
  3. Implement against the Goal
  4. Use /prove before you report done, writing report.md to the Evidence directory the
     dispatch named. A failing item is fixed and re-proved, never reported as progress

## 4. Dispatch independent Subagents at once

### Dispatch a roster Agent, never a Harness built-in
`subagent_type` names an Agent from the roster. A Hook refuses `Explore`, `Plan`, and `general-purpose`, and the refusal kills the whole parallel batch: every dispatch in that message comes back as a failed row.

### Run independent Tasks in parallel
One message, multiple Agent calls, each naming its Evidence directory
(docs/agents/<YYYYMMDD>-<task-slug>/). Every dispatch is already async and parallel, so it
returns its agentId at once and you keep working. Sequence only when one Task's output
feeds the next.

### Dispatch without a name
An unnamed dispatch returns its report and resumes by agentId; `block_builtin_subagents.py`
refuses a named one.
Never: `name`, or `run_in_background`, which the Agent tool has no parameter for.

### Resume only to finish or correct the dispatched Task
`SendMessage({to: agentId})` resumes an agent from its transcript, even after it returned,
using the agentId from its spawn result. A resume finishes or corrects that agent's own
founding Task, nothing else. A new finding, failure, or scope item — even on the same
surface — is a fresh dispatch: one agent, one task, and a clean Context beats a warm one.

Never: a resume carrying work the founding Prompt did not name, or "it already has
context" as the reason to route new work to an old agent.

### Recover a lost agentId from disk
`~/.claude/projects/<project>/<session>/subagents/*.meta.json` names every Subagent the
session dispatched, with its agentType and model. Read it when the id has left your
Context — the agent is still resumable.

### Check on Agents; never put them on a timer
On the coordination cadence, read a running agent's Evidence directory; when nothing moved,
SendMessage it by agentId for status. Only when resume fails — including an agent whose Context is
exhausted and resumes into silence — dispatch a replacement implementing Subagent with the original
Prompt, the current diff, and the Evidence directory.

Never: kill or time out an agent still working the Task. Stop one only when you have abandoned
its Task, and record the abandonment.

IF one agent has failed three fix attempts on one bug:
### Stop resuming and dispatch a debugger for the mechanism
Further resumes buy guesses. Dispatch a debugger to capture the failing artifact — the compared pair, the stack, the revision pair — and route the diagnosed fix with its file:line Evidence.

## 5. Verify, hold the Goal, and orchestrate

### Preserve Context for coordination
Context spent on the work is Context not held for the big picture. Doing a Task yourself, or reading a wide diff yourself, is why the Task goes to a Subagent. The Review is the one exception, and /review owns it: you read the changeset there, because a merge of reports nobody checked is not a Review.

IF the Decision a dispatch rests on is still open with the Architect:
### Hold the dispatch until the Decision closes
Dispatching mid-interrogation executes an unapproved change. The Architect is still questioning the Decision, so no Subagent starts on it until he closes it.

IF a second Subagent is running on the surface you are about to correct:
### Stop it before sending the correction
Two live Subagents on one surface collide: each overwrites what the other wrote. Stopping the duplicate abandons its Task, so record the abandonment, then send the correction to the one Owner.

IF the Architect's words hold and change a thing the Orchestrator owns:
### Translate owned feedback into the dispatch
The Goal, the Architecture, and coordination are the Orchestrator's. Fold the feedback
into the Goal and Architecture blocks. A correction to work in flight rides a resume; a
changed Goal founds a fresh dispatch.

IF the Architect gives feedback on a thing a Subagent owns:
### Relay the Architect's words to the Owner
Quote the feedback verbatim in the resume message and add the Context the Owner lacks —
prior calls, boundaries, sibling work. The Owner makes the decisions about its craft;
a translated version replaces the Owner's judgment with the Orchestrator's and distorts
what the Architect said.

Never: turning "the spacing feels cramped" into "set the gap to 16px" for the designer;
adding fixes, preferences, or decisions the Architect never gave.

