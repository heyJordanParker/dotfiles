---
name: bug-hunting
description: Use when reviewing code for bugs, logic errors, security vulnerabilities, edge cases, race conditions, and resource leaks. Focuses on correctness - does the code do what it claims?
---

# Bug Hunting

Gate for catching defects before they reach production.

**Core principle:** Code does what it claims, handles what it encounters, and fails loudly when it can't.

## Categories

### 1. Logic Errors

- **Off-by-one** - Loop bounds, array indices, range checks
- **Wrong operator** - `=` vs `==`, `&&` vs `||`, `<` vs `<=`
- **Inverted condition** - Checking opposite of intent
- **Missing negation** - Forgot `!` or `not`
- **Short-circuit issues** - Side effects in conditions that may not run
- **Operator precedence** - Missing parentheses changing meaning

**Red flags:**
- Loops with `<` or `<=` on boundaries
- Complex boolean expressions
- Conditionals without else branches

### 2. Null/Undefined Handling

- **Unguarded access** - `obj.prop` when obj might be null
- **Optional chaining gaps** - `a?.b.c` (c not protected)
- **Falsy confusion** - `0`, `""`, `false` treated as missing
- **Missing default** - No fallback for undefined values

**Red flags:**
- Chained property access without guards
- Array indexing without bounds check
- Object destructuring without defaults

### 3. Security Vulnerabilities

- **Injection** - SQL, command, template injection
- **XSS** - Unescaped user input in output
- **Auth bypass** - Missing permission checks
- **Path traversal** - `../` in file paths
- **Insecure deserialization** - Trusting serialized data
- **Secrets exposure** - Keys/passwords in code or logs

**Red flags:**
- String concatenation with user input
- `dangerouslySetInnerHTML`, `eval`, `exec`
- Missing auth middleware on routes
- Hardcoded credentials

### 4. Edge Cases

- **Empty collections** - Array/list with 0 items
- **Single item** - Collection with exactly 1 item
- **Boundary values** - Max int, empty string, whitespace
- **Unicode** - Emoji, RTL text, special characters
- **Timezone issues** - Date comparisons across zones
- **Floating point** - Precision loss in comparisons

**Red flags:**
- `array[0]` without length check
- Date arithmetic without timezone handling
- `==` for float comparison

### 5. Race Conditions

- **Check-then-act** - State changes between check and use
- **Read-modify-write** - Non-atomic operations
- **Shared mutable state** - Multiple writers without sync
- **Async ordering** - Assumptions about completion order
- **Stale closures** - Capturing old values in callbacks

**Red flags:**
- `if (exists) { use() }` with async operations
- Counter increments without locks
- Setting state in async callbacks

### 6. Resource Leaks

- **Unclosed handles** - Files, connections, streams
- **Missing cleanup** - Event listeners, timers, subscriptions
- **Memory leaks** - Growing caches, circular references
- **Connection exhaustion** - Not returning to pool

**Red flags:**
- `open()` without corresponding `close()`
- `addEventListener` without `removeEventListener`
- `setInterval` without `clearInterval`
- `subscribe` without `unsubscribe`

### 7. Error Handling Gaps

- **Unhandled promise rejection** - Missing `.catch()` or try/catch
- **Swallowed exceptions** - Empty catch blocks
- **Wrong error type** - Catching too broadly
- **Missing finally** - Cleanup that must run

**Red flags:**
- Async function without error handling
- `catch (e) {}` or `catch (e) { log(e) }`
- `catch (Exception e)` catching everything

## Process

1. **Trace data flow** - Where does input come from? Where does output go?
2. **Check boundaries** - What happens at edges? Empty? Max? Null?
3. **Verify assumptions** - What must be true for this to work?
4. **Test failure paths** - What happens when things go wrong?
5. **Look for side effects** - What state changes? Is it safe?

## Quick Reference

| Bug Type | What to Check |
|----------|---------------|
| Logic | Operators, conditions, loop bounds |
| Null safety | Every `.` access, array index |
| Security | User input paths, auth gates |
| Edge cases | Empty, single, boundary values |
| Race conditions | Shared state, async operations |
| Resource leaks | Open/close pairs, subscriptions |
| Error handling | Every async call, every throw point |
