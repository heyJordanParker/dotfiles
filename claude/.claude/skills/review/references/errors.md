# Error Handling Review

Dispatch silent-failure-hunter agent to audit error handling.

**Focus:** Empty catches, swallowed errors, silent fallbacks, inadequate logging.

## When to Use

- Code with try/catch blocks
- Error handling refactors
- Integration points (APIs, DB, file I/O)
- Before merging code that handles failures

## How to Request

**1. Get changed files:**
```bash
git diff --name-only HEAD~1  # or appropriate range
```

**2. Dispatch silent-failure-hunter:**

Use Task tool with `pr-review-toolkit:silent-failure-hunter` subagent type:

```
Review error handling in these changes:

Files: {CHANGED_FILES}
Git range: {BASE_SHA}..{HEAD_SHA}

Focus on:
- Empty catch blocks
- Errors logged but not propagated
- Silent fallbacks that hide failures
- Missing error context
- Catch-all handlers that swallow specifics
```

**3. Act on findings:**
- Fix silent failures immediately
- Add proper error propagation
- Ensure errors surface to callers

## What It Catches

- `catch (e) {}` - Silent failure
- `catch (e) { log(e) }` - Logged but swallowed
- `?? defaultValue` - Silent fallback
- `try { } catch { return null }` - Error hidden as valid state
- `catch (Error e)` - Overly broad, loses specifics

## Integration

Use after anti-slop check finds §6 (Silent failures) issues for deeper audit.
