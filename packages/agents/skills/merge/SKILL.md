---
name: merge
description: An Agent merges two branches one file at a time and checks each file against its two parents, so both sides' code survives while one side's feature stops working and no file-level check sees it. This Skill merges the branches and proves every behaviour each side committed still runs afterward. TRIGGER when merging two branches that both carry behaviour-changing commits since the merge base, on "/merge", "audit the merge", and whenever a behaviour is missing from a tree that was already merged. DO NOT TRIGGER when one side changed no behaviour since the base, meaning a fast-forward or a side whose commits only touch documentation, formatting, or comments.
---

# Merge

- A merge keeps both branches' code changes while the merged code no longer performs one branch's behaviour: one side's change in file A stops the other side's change in file B from taking effect, and a per-file check does not detect it.
- Every behaviour each side claims must survive, and the owning branch has the last word on its own.

## 1. Resolve the merge facts

### Take each side's commit list from the merge base
`git log <base>..<tip>` for each side.

IF a side is one squashed commit:
### Split it by the sections of its commit body

### Derive the owner map from the commit bodies and dates
Never: asking the Architect which branch owns a subsystem.

### Judge whether the two sides can reach each other before you dispatch anything
Each side's changed files come from `git diff --name-only <base>..<tip>`, and `trace downstream`
and `trace upstream` show what reaches what. Two sides that never change the same code, and never
change code that reaches the same code, have nothing to compose: merge, run the test suite, and
record in the report that no claim audit ran and what showed it. The graph carries no style or
template coupling, so judge that from the files themselves.
Never: a commit count or a changed-file count as the test. One file in the editor shell is enough
to stop the other side's loader.

## 2. Dispatch one `architect` for each branch

### Dispatch exactly two `architect` agents, one per branch
Use /delegate for the dispatch.
Never: one agent per commit, one per pass, or another agent type.

### Write each side's claim blocks to its Evidence directory yourself
The `architect` declares `readonly: true` and has no write tool on either Harness, so the blocks
come back in its reply and the file is yours to write.

Template for the dispatch's Process block:
  ```
  1. Run /understand over this branch's commits since <base>, so your claims come from a
     reader who knows why each hunk exists. It is your reading, not a deliverable.
  2. Write one claim per behaviour from the commit bodies and the diffs. A claim is a
     behaviour, never a file change: a rename-only hunk is one claim, and a hunk that
     removes a capability is a claim. Map every hunk to a claim, and report an unmapped
     hunk as a finding. Return one block per claim, verdict lines empty:

     ### <side>-<n> <the behaviour in one sentence, in the commit's own words>
     - source: <commit> body line "<quote>" | <commit> hunk <path>@<symbol>
     - carried by: <path>:<symbol>, ...   (every file on the path in this branch)
     - consumers: <what calls or renders it in this branch>
     - observable: <the read target, and the runtime check where one exists>
     - verdict: HOLDS | LOST | ALTERED | UNVERIFIED
     - read: <path>:<lines> at the merge commit, what it does
     - collision: <path> changed by <other branch>: <what the composition does> | none
     - run: <test or screenshot path and result> | not runnable
     - cause: <the hunk that stops it from taking effect, path:symbol>   (LOST and ALTERED only)

  3. When your Queue names the merge commit, fill the verdict lines from it. `trace read
     <path> --at <merge-commit>` reads the merged code; never read the working tree, which
     other Agents edit while you work. Start with the hand-resolved files the Queue names.
     Read the level above and the level below each carrier, then check whether <other-tip>
     changed that level: a loading gate the other branch adds in the shell runs before this
     branch's in-body loading state, and a wrapper element it inserts stops this branch's
     child-combinator style rule from matching. Every path <other-tip> also changed is a
     collision site: `git diff <base> <other-tip> -- <path>` names them, and you read the
     composed code and state what runs. HOLDS needs every carrier read plus the observable
     run, which /prove governs. A claim with no run is UNVERIFIED.
  4. Return the claim blocks with their verdict lines filled, and nothing else.
  ```

## 3. Merge

### Make the merge yourself, once both claim sets are in hand
A claim written after the merged code is read is a claim the merged code already satisfies.

IF the merge commit already exists:
### Audit it as it stands

### List the files the merge resolved by hand
```bash
git merge-tree --write-tree <a-tip> <b-tip>
git diff-tree -r --name-status <that-tree> <merge-commit>
```

### Account for every file the pair returns
Conflict resolution, renumbered migration, or follow-through of the other side's rename, each read
in the diff yourself. A file you cannot place is a finding.

## 4. Resume each dispatch to verify

### Put the merge commit and that side's hand-resolved files in the Queue
Git's automatic merge did not change any other file, so a claim carried by a hand-resolved file is
the most likely one to be lost.

## 5. Gate the verdicts, report, and fix

### Read every LOST, ALTERED, and UNVERIFIED verdict at the merge commit yourself
An `architect`'s verdict is a claim until you have read the composed code.

### Carry the owning branch on every finding you send to /triage
Which side lost the behaviour decides who fixes it.

IF a behaviour from each side collides:
### Take the collision to the Architect
Both must survive, and the shape that carries both is his call.
Never: keeping one side silently.

### Apply the fixes yourself

### Run the test suite and the build once, after the last fix lands
