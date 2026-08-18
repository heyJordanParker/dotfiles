---
name: prove
description: Produce the Evidence that proves work is done before any completion claim.
  TRIGGER before reporting a Task complete, writing report.md, or claiming fixed,
  passing, or pre-existing. DO NOT TRIGGER while still implementing.
---

# Prove

Every claim in a report is false until its Evidence exists. The report is written from
the Evidence, never the other way around.

## Process

1. List every claim the report will make: each Verification item, each "fixed", each
   "passes", each "pre-existing".

2. Derive what the change reached, and add each consumer to the claim list. The
   dispatch's Verification block was written before anyone looked, so it names what
   was expected to move, never what moved. The tree names what moved: `trace diff
   --symbols` for the symbols in the files you changed — the tree carries other
   Agents' work too, so read only your own — `trace callers` on each one, `trace grep` on
   every name you removed — in code, comments, docs, scripts, and deploy Hooks. Zero
   hits, or a reason per hit. A consumer surfacing here is read, not exercised, until
   reading shows it broke.

   ### Give every deleted capability a new home
   For each file you deleted or replaced, name what it did and where that lives now.
   A capability with no new home is a defect you fix before reporting, never a line
   in the report.

   ### Delete the second mechanism you just added
   Your new method beside the one that already did the job is a defect you fix before
   reporting, not a difference for a reviewer to find.

3. Exercise each claim with the narrowest run that reaches your own diff — one spec
   file, one test, one browser session at most, closed on exit. Full suites, test
   categories, and cross-cutting checks are the Orchestrator's single end gate; cite its
   artifacts instead of re-running them. Run the concrete input, record the observed
   output. A flow through a browser gets an ordered screenshot trail, one frame per step
   the User takes, numbered in order. A single end-state frame proves the end state only.
   Example: "POSTed `{email: a@b.co}` to `/api/v1/form-submit`; got 200; redirect to `/thanks`."
   Never: "build succeeds", "types check", "logic is sound", or a confidence percentage in place of an observed run.

   ### Iterate on one test, prove on the file
   A whole test file repeats the migration, the fixtures, and the app boot for
   every test in it, so it costs minutes where one test by name costs seconds.
   While the code is red, run one test by name. Run the whole file, the browser
   walk, or the same test twice to prove a race after that one test passes.

   IF a check already passed:
   ### Re-run it only after you changed code it covers
   A check that passed stays passed.

   IF you have no Orchestrator:
   ### Run the end gate yourself
   There are no artifacts to cite, so the full suite is yours to run.

4. Look at every screenshot and check it against the claim it backs. A screenshot
   showing the failure kills the claim — fix the work, never ship the image.
   Never: attaching a screenshot unviewed; a blank, sliver, or cropped image as proof.

5. IF a gate you own is still red:
   The Task is not done. Fix it, or report unfinished with the red output shown.
   Never: reporting movement as progress ("advanced past line 1836, now fails at 1875");
   "unrelated", "existing failure", or "was broken before" offered as a pass.

   IF a failure looks pre-existing:
   ### Report the exact command and its red output to the Orchestrator and stop
   Attribution is the Orchestrator's, not yours: it holds the diff, the other Tasks in
   flight, and the before state. With no Orchestrator, fix the red gate or surface it in
   your own report. You never produce a before state yourself, because the
   tree carries other Agents' uncommitted work and any command that clears it destroys
   their work.

6. Write report.md in the named Evidence directory. Every claim carries its input,
   observed output, and screenshot beside it, each screenshot named by its filename so
   the reader opens the trail instead of trusting the claim. A claim without proof stays out.

7. Post the completion summary: what was exercised start to finish, and what was not.
   Both are facts you hold. What an unexercised part costs in production is the
   Orchestrator's call, from the whole changeset — writing it here is a guess from
   one Task's view.
