---
name: review
description: Read the changeset yourself, review it with one Claude reviewer and one codex reviewer on the same target at once, triage their findings with /triage, and iterate until a round returns no breaking finding. TRIGGER on "review the changes", "code review", on the Review before a changeset is presented to the Architect, and when a /orchestrate or /verify-changes step names it. DO NOT TRIGGER for N identical reviewers on one Task (that is /independent-review).
---

# Review

- Two Harnesses read the same changeset, so a blind spot in one is caught by the other.
- /delegate owns the dispatch Prompt; /orchestrate owns judging what returns.

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

## 3. Triage the returns with /triage

### Append each round to the one triage.md
Each round appends its own section to the run's one triage.md, so a recurring finding is visible and a clean round is checked against the sheet, never against memory.

## 4. Re-run both reviewers on the fixed changeset

Repeat steps 1 through 3 on the updated changeset.

### Stop only on a clean round
The Review ends when one full round from both reviewers returns no breaking finding. A round that fixed something is never the last round.
