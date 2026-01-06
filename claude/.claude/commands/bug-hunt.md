---
description: Review code for bugs, logic errors, security vulnerabilities, edge cases
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

**Red flags:** Loops with `<` or `<=` on boundaries, complex boolean expressions

### 2. Null/Undefined Handling
- **Unguarded access** - `obj.prop` when obj might be null
- **Optional chaining gaps** - `a?.b.c` (c not protected)
- **Falsy confusion** - `0`, `""`, `false` treated as missing

**Red flags:** Chained property access without guards, array indexing without bounds check

### 3. Security Vulnerabilities
- **Injection** - SQL, command, template injection
- **XSS** - Unescaped user input in output
- **Auth bypass** - Missing permission checks
- **Path traversal** - `../` in file paths
- **Secrets exposure** - Keys/passwords in code or logs

**Red flags:** String concatenation with user input, `eval`, `exec`, missing auth middleware

### 4. Edge Cases
- **Empty collections** - Array/list with 0 items
- **Single item** - Collection with exactly 1 item
- **Boundary values** - Max int, empty string, whitespace
- **Unicode** - Emoji, RTL text, special characters
- **Floating point** - Precision loss in comparisons

**Red flags:** `array[0]` without length check, `==` for float comparison

### 5. Race Conditions
- **Check-then-act** - State changes between check and use
- **Read-modify-write** - Non-atomic operations
- **Stale closures** - Capturing old values in callbacks

**Red flags:** `if (exists) { use() }` with async operations, setting state in callbacks

### 6. Resource Leaks
- **Unclosed handles** - Files, connections, streams
- **Missing cleanup** - Event listeners, timers, subscriptions

**Red flags:** `open()` without `close()`, `addEventListener` without `removeEventListener`

### 7. Error Handling Gaps
- **Unhandled promise rejection** - Missing `.catch()` or try/catch
- **Swallowed exceptions** - Empty catch blocks
- **Missing finally** - Cleanup that must run

**Red flags:** Async function without error handling, `catch (e) {}`

## Process

1. **Trace data flow** - Where does input come from? Where does output go?
2. **Check boundaries** - What happens at edges? Empty? Max? Null?
3. **Verify assumptions** - What must be true for this to work?
4. **Test failure paths** - What happens when things go wrong?
5. **Look for side effects** - What state changes? Is it safe?
