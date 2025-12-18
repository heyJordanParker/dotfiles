---
description: Validated commit with tests and comprehensive review
---

Final gate when feature is done and user-tested.
Use `/code-review` during development after architectural changes.

## Step 1: Pre-flight

Run `git status --porcelain`.

- No staged changes but unstaged exist: "Stage files first: `git add <files>`" → exit
- Nothing to commit: "Nothing to commit." → exit

## Step 2: Tests (Hard Block)

Run `bun test`.

- **Pass:** Continue
- **Fail:** Show output, "Tests failing. Fix before committing." → exit

## Step 3: Review

Launch 6 subagents in parallel using the Task tool. Each reviews `git diff --cached` (staged changes only).

### Subagent 1: Bug Hunting

```
You are a bug hunter. Use the `bug-hunting` skill.

1. Run `git diff --cached` for staged changes
2. Hunt for: logic errors, security vulnerabilities, edge cases, race conditions
3. Report findings:

**Critical:** (security vulnerabilities, data loss risks)
**Important:** (logic errors, unhandled edge cases)
**Minor:** (potential issues)

If clean: "No bugs found."
```

### Subagent 2: Anti-Slop

```
You are an anti-slop reviewer. Use the `anti-slop` skill.

1. Run `git diff --cached` for staged changes
2. Scan all 13 slop categories
3. Report findings:

**Critical:** (security holes, silent failures, hallucinated deps)
**Important:** (type escapes, YAGNI, duplication, test slop)
**Minor:** (comment slop, style, dead code)

If clean: "No slop found."
```

### Subagent 3: Naming

```
You are a naming reviewer. Use the `naming` skill.

1. Run `git diff --cached` for staged changes
2. Review for naming issues
3. Report findings:

**Critical:** (naming that causes confusion or bugs)
**Important:** (naming that hurts readability)
**Minor:** (naming suggestions)

If clean: "No naming issues found."
```

### Subagent 4: Architecture

```
You are an architecture reviewer. Use the `architecture-review` skill.

1. Run `git diff --cached` for staged changes
2. Check: SOLID violations, encapsulation breaks, dependency direction
3. Report findings:

**Critical:** (circular dependencies, major SOLID violations)
**Important:** (encapsulation breaks, wrong dependency direction)
**Minor:** (separation of concerns suggestions)

If clean: "No architecture issues found."
```

### Subagent 5: Pragmatic Engineering

```
You are a pragmatic engineering reviewer. Use the `pragmatic-engineering` skill.

1. Run `git diff --cached` for staged changes
2. Check: YAGNI, over-engineering, unnecessary complexity
3. Report findings:

**Critical:** (building what shouldn't exist)
**Important:** (unnecessary abstraction, complexity)
**Minor:** (could be simpler)

If clean: "Code follows pragmatic principles."
```

### Subagent 6: Verification

```
You are a verification reviewer. Use the `verification-before-completion` skill.

1. Run `git diff --cached` for staged changes
2. Check for: untested claims, deleted tests, missing test coverage for new code
3. Report findings:

**Critical:** (deleted tests for existing code, security code without tests)
**Important:** (new functionality without tests, error handling that hides failures)
**Minor:** (coverage suggestions)

If clean: "Changes properly verified."
```

## Step 4: Gate

Aggregate findings from all reviewers:

```
# Commit Review

## Critical
[issues]

## Important
[issues]

## Minor
[issues]
```

- **Any Critical:** Block. "Fix critical issues before committing." → exit
- **Any Important:** Ask: "Fix these or proceed anyway?"
- **Minor only:** Report and continue

## Step 5: Commit

1. Run `git log --oneline -5` for style reference
2. Analyze `git diff --cached`
3. Generate message:
   - `type: subject` (<72 chars)
   - Body explaining WHY
   - Types: fix, feat, chore, refactor, docs
4. Auto-commit with generated message
5. User can amend after: `git commit --amend`

## Step 6: Verify

1. Verify exit code 0
2. Run `git status` to confirm clean state
3. Report: "Committed: [SHA] [subject]"

## Checklist

Before completing, verify:
- [ ] Tests pass (fresh run, not cached)
- [ ] No critical issues from any reviewer
- [ ] Important issues addressed or acknowledged
- [ ] Commit message explains WHY
- [ ] git status confirms clean commit
