---
description: Validated commit with tests and comprehensive review
---

Final gate when feature is done and user-tested.

## Step 1: Pre-flight

Run `git status --porcelain`.

- Unstaged changes exist: Stage all changes
- Nothing to commit: "Nothing to commit." → exit

**Sanity check after staging:** Review what's staged. Warn and confirm if anything looks off—secrets, credentials, unrelated files, or anything that doesn't belong in this commit.

## Step 2: Tests (Hard Block)

Run `bun test`.

- **Pass:** Continue
- **Fail:** Show output, "Tests failing. Fix before committing." → exit

## Step 3: Review

Invoke `/review` skill. It runs 8 parallel subagents on staged changes.

Apply gate from review results:
- **Any Critical:** Block. "Fix critical issues before committing." → exit
- **Any Important:** Ask: "Fix these or proceed anyway?"
- **Minor only:** Report and continue

## Step 4: Commit

1. Run `git log --oneline -5` for style reference
2. Analyze `git diff --cached`
3. Generate message (use the `commit-messages` skill)
4. Auto-commit with generated message
5. User can amend after: `git commit --amend`

## Step 5: Verify

1. Verify exit code 0
2. Run `git status` to confirm clean state
3. Report: "Committed: [SHA] [subject]"

## Checklist

Before completing, verify:
- [ ] Tests pass (fresh run, not cached)
- [ ] No critical issues from review
- [ ] Important issues addressed or acknowledged
- [ ] Commit message explains WHY
- [ ] git status confirms clean commit
