---
name: triage
description: Turn returned reviewer findings into one graded report grouped by who each problem affects, and act on the gate. TRIGGER after review Subagents return findings — from /review, /regressions, /user-testing, or any audit — and before results reach the Architect. DO NOT TRIGGER while reviewers are still running, or for a single finding routed directly to its owner.
---

# Triage

- Input is every reviewer report returned this run.
- Output is one report grouped by who each problem affects, with the gate acted on.
- A finding says who the problem affects — the User, the Architecture, or the business — and what breaks for them.

## 1. Collect every finding

Pull every finding from every returned report into one list, keeping each finding's source and place.

## 2. Assess each finding against the code, writing the sheet as you go

### Confirm the mechanism yourself before the finding gets a grade
Open the file the finding names and confirm the call, the missing check, or the broken path. A finding is the reviewer's claim; the grade is yours. Drop a finding the code does not show, with its one-line reason.
Never: grade or route a finding without confirming it against the code.

### Write each verdict to triage.md the moment you reach it
The sheet is docs/agents/<YYYYMMDD>-<task-slug>/triage.md in the run's Evidence directory: one row per finding — source, what the code showed, grade, ruling. Append the row as the finding is assessed, never in a batch at the end, so the write records the judgment instead of replacing it.

## 3. Rewrite each finding as who it affects and what breaks

### Drop a finding that names no one affected and nothing broken
Record the one-line reason it was dropped.
Example: "`MediaController` swallows the upload exception" becomes "the User's upload fails and no error is shown".
Never: keeping a finding because the reviewer sounded confident.

## 4. Merge duplicates

Two reviewers reporting the same underlying problem merge into one finding at the highest severity, keeping both sources.

## 5. Grade each finding on what breaks

### Grade on what breaks, never on how the reviewer sounded
- Blocking: something no longer works — the User can no longer do X, the system can no longer do Y, or a Critical Path breaks.
- Important: it still works but costs — a worse experience, a dependency pointing the wrong way, wasted spend.
- Polish: a rough edge where nothing stops working.
Never: grading by reviewer confidence, finding count, or how alarming the wording is.

## 6. Act on the gate

Only findings confirmed in step 2 pass the gate. Blocking fails the run: route each Blocking finding to the Subagent that owns the fix, per /delegate, before anything is presented as done. Important is reported to the Architect. Polish is noted and does not block.

### Route the cause /5-whys reaches, never the finding as it arrived
A fix dispatched at a symptom returns the same cause as a new finding next round.

IF the fix needs an Architectural change:
### Take it to the Architect with /pcc
The Architect owns the Architecture, so the gate stops at the Decision instead of making it.

## 7. Report grouped by who it affects

Template:
    # Findings

    ## User
    - Blocking — [path] — [what the User can no longer do]
    - Important — [path] — [what got worse for the User]

    ## Architecture
    - Important — [path] — [what drifts or duplicates]

    ## Business
    - Polish — [path] — [what it costs]

    Dropped: [finding] — [why the code does not show it, or why it named no one affected and nothing broken]

    If all clear: "No findings."
