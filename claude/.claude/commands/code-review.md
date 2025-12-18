---
description: Quick code review with 3 parallel reviewers
---

Reviews all uncommitted changes (staged + unstaged) after architectural changes.
Use `/commit` when feature is done and user-tested.

## Instructions

Launch 3 subagents in parallel using the Task tool. Each subagent should:
1. Run `git diff HEAD` to get uncommitted changes
2. Use its assigned skill to review the code
3. Report findings in severity buckets: Critical / Important / Minor

### Subagent 1: Naming Review

```
You are a naming reviewer. Use the `naming` skill.

1. Run `git diff HEAD` to get uncommitted changes
2. Review all code for naming issues
3. Report findings:

**Critical:** (naming that causes confusion or bugs)
**Important:** (naming that hurts readability)
**Minor:** (naming suggestions)

If no issues: "No naming issues found."
```

### Subagent 2: Bug Hunting

```
You are a bug hunter. Use the `bug-hunting` skill.

1. Run `git diff HEAD` to get uncommitted changes
2. Hunt for: logic errors, security vulnerabilities, edge cases, race conditions, resource leaks
3. Report findings:

**Critical:** (security vulnerabilities, data loss risks)
**Important:** (logic errors, unhandled edge cases)
**Minor:** (potential issues, suggestions)

If no issues: "No bugs found."
```

### Subagent 3: Pragmatic Engineering Review

```
You are a pragmatic engineering reviewer. Use the `pragmatic-engineering` skill.

1. Run `git diff HEAD` to get uncommitted changes
2. Review against:
   - Are we building something that shouldn't exist?
   - Is this over-engineered? Could it be simpler?
   - Abstractions without 3+ use cases?
   - Duplicating 3rd party functionality?
3. Report findings:

**Critical:** (building what shouldn't exist, major over-engineering)
**Important:** (YAGNI violations, unnecessary complexity)
**Minor:** (could be simpler)

If no issues: "Code follows pragmatic principles."
```

## Aggregation

After all subagents complete:

```
# Code Review

## Critical
[issues]

## Important
[issues]

## Minor
[issues]
```

If all clear: "No issues found."

## Gate

- **Critical issues:** Report to user. Do not proceed to commit.
- **Important/Minor:** Report and continue.

## Checklist

- [ ] Naming: No confusing or inconsistent names
- [ ] Bugs: No security issues or logic errors
- [ ] Pragmatic: No YAGNI violations or over-engineering
