---
name: revise
description: The writer's own draft loop — draft from the plan, fold findings to locally clean, then answer a review round's blocking findings; a rejected draft is never a seed. TRIGGER when a writer folds findings into their draft, in the local production loop before review or on a review round's blocking findings. DO NOT TRIGGER to run the checks that produce findings (review) or to judge a finished piece (the check skills).
---

# Revise

One Process: the writer drives their own piece from a first draft to a clean one. The writer who owns a piece is the sole author of its `drafts/Draft-NNN.md` and its sole reviser — findings come from the checks, the fix comes from the author. No check ever mutates the draft: the line passes (edit-sentences, check-structure, check-ai-writing) return findings and suggested edits, and revise is where the writer folds every accepted fix into the next numbered draft.

## 1. Draft from the plan and loop to locally clean

### Draft from the plan files, then fold to locally clean before review
The writer drafts from Reader.md, Brief.md, Proof.md, and Voice.md — a campaign piece reads the campaign's shared Brief.md and Proof.md and its own per-piece Reader.md. Fold the accepted line-pass findings into the next numbered draft and loop until the piece is locally clean, before it goes to review. copycheck is the editor's to run in the review round (review-copy), never the writer's.

### Confirm Voice.md carries a sample before drafting
Before the first draft, check Voice.md carries at least one owner copy sample. A sample-free Voice.md is an owner question — flag it in `OpenQuestions.md` and surface it before drafting, never match a voice against samples that do not exist. setup's Voice.md contract owns the same flag at capture time; this is the writer's last check that it was answered.

## 2. Group and triage

### Group findings by piece, then split blocking from note
For each piece, sort the round's findings into blocking (holds the piece) and note (record, does not block). Only blocking findings force a revision this round.

## 3. Revise or rewrite

### Answer small findings in place; rewrite on a big miss
Where the findings are local, revise the exact lines and sections they name. Where the findings show the draft missed the brief — wrong audience, wrong argument, wrong offer — throw it out and write fresh.
Never: reuse a rejected draft as the seed of the next one. A draft the checks rejected is dead with its hash; a big miss means a new write, not a patch.

## 4. Hand back for rerun

### Return the revised draft with the findings it answered
Return the new draft naming which findings it resolved, so review-copy reruns only the affected checks against the new hash.

Verification: the piece drafted from its plan files and looped to locally clean before review; every blocking finding either answered in the draft or escalated; no rejected draft carried forward as a seed; the revised draft named against the findings it resolved.
