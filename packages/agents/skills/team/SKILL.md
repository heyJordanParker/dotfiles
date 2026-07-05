---
name: team
description: Framework for creating and coordinating persistent teams. Teams persist across Slices and turns; teammates accept new work via SendMessage. Covers team Process, dispatch patterns, Verification, and the /subagents ranking Process. For one-shot Subagents, use /subagents.
---

# Team

Persistent teams coordinate teammates across turns and Slices.
The coordinator preserves Context for coordination; teammates write every line of code.
Every dispatch follows the /subagents Prompt Template: Story, Business, Goal, Verification, Architecture, Process.

## 1. Create a team only for three or more Tasks

### Create with TeamCreate
Use `TeamCreate(team_name, description)` for three or more Tasks.

IF the Goal has fewer than three Tasks:
### Use one-shot Subagents
Use /subagents with the same Prompt Template. Do not use TeamCreate or TaskCreate.

### Never close teams
Teams run indefinitely; the Architect decides when done. Closing destroys accumulated Context that costs real money to rebuild.

Never: TeamDelete or shutdown calls.

## 2. Decompose the Goal into independent Tasks

### Use TaskCreate for each Task
Set `activeForm` present-continuous for the User's progress spinner. Each Task must be completable with no knowledge of the others.

## 3. Spawn specialized teammates

### Match the teammate to the domain
Use `code-reviewer`, `architect`, `backend-engineer`, `frontend-engineer`, `researcher`, or `tester`; never general-purpose.

### Keep two to four active teammates
Reuse existing teammates via SendMessage rather than spawning more.

### Never spawn replacements for failed teammates
SendMessage feedback to the same teammate so it iterates with full Context.

### Run teammates in the background
Use `run_in_background: true`.

Template:
  Agent(subagent_type: "backend-engineer", team_name: "feature", name: "worker",
        prompt: "Story, Business, Goal, Verification + Architecture + Process",
        run_in_background: true)

## 4. Coordinate without polling

### Wait for teammate notifications
Teammates notify you on completion or blockers. Respond via SendMessage and track via TaskList. Idle is normal between turns; SendMessage wakes teammates.

Never: check in, poll, or ask for status.

### Express dependencies with TaskUpdate
For dependent work, set `TaskUpdate` `blockedBy`. For independent work, spawn all teammates at once and watch for file conflicts.

### Weave research into implementation Prompts
For research → implement, digest the research and weave it into the implementation Prompt's Story or Business section.

## 5. Review teammate output

### Verify, hold the Goal, and coordinate
A teammate summary is a claim. Prove it by exercising the User-facing Critical Path with a concrete input and observed output.

### Preserve coordination Context
Delegate every code change, including "just a small fix". Read teammate summaries and spot-check; do not read full implementation files.

Never: Edit, Write, NotebookEdit, or reading massive changesets yourself.

### Dispatch larger Verification
Under 200 lines of code may be read directly; larger work gets a Verification Subagent. Then dispatch a `code-reviewer` against Verification, accept the work, or SendMessage specific feedback.

### Reject incomplete teammate claims
Reject scope cop-outs, broken-before deflections, blocked excuses, and shallow reframes. A teammate finding that contradicts the Architect's reported outcome means the teammate is incomplete, not the Architect.

### Restore by hand
The teammate that broke a file restores it by hand, or the Architect does. Destructive-git restore is banned.

### Take the long hard way
Read then edit, file by file. Do not use bulk-rewrite scripts or AI Slop shortcuts.

## 6. Run the ranking Process from /subagents

### Rank in the coordinator's voice
Strip the recommendation, finish the research, eliminate, rank with /pcc, and recommend in your own voice.

### Send thin work back to the same teammate
Thin work goes back to the same teammate via SendMessage, never a fresh dispatch. Include the /subagents research closing paragraph in research Prompts.

## 7. Integrate

### Verify the whole changeset
After all Tasks, verify no conflicts between teammate outputs, run full Verification, and dispatch a final review Subagent across the whole changeset.
