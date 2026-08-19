---
name: review
description: Read the changeset yourself, review it with one Claude reviewer and one codex reviewer on the same target at once, merge their findings into your own, and iterate until a round returns no breaking finding. TRIGGER on "review the changes", "code review", on the Review before a changeset is presented to the Architect, and when a /orchestrate or /verify-changes step names it. DO NOT TRIGGER for N identical reviewers on one Task (that is /independent-review).
---

# Review

- Two Harnesses read the same changeset, so a blind spot in one is caught by the other.
- /delegate owns the dispatch Prompt and the Evidence bar.

## 1. Read the changeset yourself first

### Read every changed file against the Architecture before either reviewer is dispatched
Your own findings are the baseline the two returns are measured against, so a problem neither reviewer saw still surfaces.
Never: a Review whose every finding arrived in a reviewer's report.

## 2. Dispatch both reviewers on one target

Send one Claude Subagent per /delegate and one codex Subagent per /codex, in the same message, both given the same target and the same Verification.

### Give both reviewers the identical target
Same diff, same surrounding code, same Prompt. A difference in scope makes the two returns incomparable.

### Require a cause, never a symptom
The dispatch tells each reviewer to run /5-whys on every finding before reporting it, and to report the cause its chain reached. A reviewer that reports what it saw sends you one round per symptom of the same cause.

## 3. Merge the findings yourself

### Never forward a reviewer's words onward
The reviewers report to you. You verify each finding against the code and write the merged result in your own voice.
Never: pasting a reviewer's report to the Architect or into the next dispatch.

### Classify each finding as breaking or not
A breaking finding is one that loses a capability, breaks the Critical Path, or contradicts the Architecture. Everything else is recorded, not fixed.

## 4. Route each breaking finding

### Run /5-whys before you route
Route the cause the chain reaches, never the finding as it arrived. A fix dispatched at a symptom returns the same cause as a new finding next round.

IF the fix leaves the Architecture unchanged:
### Dispatch it back to the owning Subagent
The Subagent that wrote the surface fixes it.

IF the fix needs an Architectural change:
### Take it to the Architect with /pcc
The Architect owns the Architecture, so the Review stops at the Decision instead of making it.

## 5. Re-run both reviewers on the fixed changeset

Repeat steps 1 through 4 on the updated changeset.

### Stop only on a clean round
The Review ends when one full round from both reviewers returns no breaking finding. A round that fixed something is never the last round.
