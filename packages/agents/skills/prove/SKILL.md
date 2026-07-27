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

2. Exercise each claim: run the concrete input, record the observed output. Behavior
   visible in a browser gets a screenshot.
   Example: "POSTed `{email: a@b.co}` to `/api/v1/form-submit`; got 200; redirect to `/thanks`."
   Never: "build succeeds", "types check", "logic is sound", or a confidence percentage in place of an observed run.

3. Look at every screenshot and check it against the claim it backs. A screenshot
   showing the failure kills the claim — fix the work, never ship the image.
   Never: attaching a screenshot unviewed; a blank, sliver, or cropped image as proof.

4. IF calling any failure pre-existing:
   Prove it with the same gate on the pre-change baseline: the exact failing command,
   green before your diff, red after. Without that run the failure is yours — fix it.
   Never: "unrelated", "existing failure", "was broken before" without the baseline run.

5. IF a gate you own is still red:
   The Task is not done. Fix it, or report unfinished with the red output shown.
   Never: reporting movement as progress ("advanced past line 1836, now fails at 1875").

6. Write report.md in the named Evidence directory. Every claim carries its input,
   observed output, and screenshot beside it. A claim without proof stays out.

7. Post the completion summary: what was exercised start to finish, what was not
   exercised, and what breaks if production hits the unexercised part.
