# Naming Review

Dispatch naming-analyzer agent to audit identifier quality.

**Focus:** Clarity, consistency, conventions, misleading names.

## When to Use

- Large refactors with new identifiers
- Code reviews with naming concerns
- When onboarding finds names confusing
- Domain modeling with new terminology

## How to Request

**1. Get changed files:**
```bash
git diff --name-only HEAD~1
```

**2. Dispatch general-purpose agent with naming skill:**

Use Task tool with `general-purpose` subagent type:

```
Review naming in these changes using the naming skill:

Files: {CHANGED_FILES}
Git range: {BASE_SHA}..{HEAD_SHA}

Check for:
- Misleading names (name doesn't match behavior)
- Inconsistent conventions (camelCase vs snake_case mixing)
- Abbreviation abuse (unclear acronyms)
- Generic names (data, info, result, handler)
- Length issues (too short or too long)
- Semantic mismatches (getX that mutates, isX that returns non-boolean)

For each issue:
- File:line reference
- Current name
- Problem
- Suggested alternative
```

**3. Act on findings:**
- Fix misleading names immediately (they cause bugs)
- Standardize convention violations
- Expand unclear abbreviations

## What It Catches

- **Misleading:** `getUser()` that creates if missing
- **Generic:** `processData()`, `handleEvent()`
- **Inconsistent:** `userId` and `user_name` in same file
- **Abbreviated:** `usrAccMgr` instead of `userAccountManager`
- **Semantic mismatch:** `isValid()` returns error message
