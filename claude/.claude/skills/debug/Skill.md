---
name: debug
description: Use when encountering bugs, test failures, or unexpected behavior. Combines systematic debugging framework, root cause tracing, and defense-in-depth validation.
---

# Debug

Framework for finding and fixing bugs correctly.

## Triggers

- Any bug, test failure, or unexpected behavior
- Errors deep in the stack
- Need to trace data flow
- Invalid data causing failures

## Core Principle

**ALWAYS find root cause before attempting fixes.** Symptom fixes are failure.

## Approach Selection

- **First error** — [systematic.md](references/systematic.md) — any bug, start here
- **Deep stack error** — [root-cause-tracing.md](references/root-cause-tracing.md) — error far from source
- **After fix** — [defense-in-depth.md](references/defense-in-depth.md) — add multi-layer validation

## Quick Reference

### The Five Phases (Systematic)

- **Phase 0: Basic Checks** — rule out typos, wrong values, copy-paste errors
- **Phase 1: Root Cause** — read errors, reproduce, gather evidence
- **Phase 2: Pattern** — find similar working code, compare
- **Phase 3: Hypothesis** — single theory, test minimally
- **Phase 4: Implementation** — failing test, single fix, verify

### Tracing Process

1. Observe symptom
2. Find immediate cause
3. Ask: What called this?
4. Keep tracing up
5. Find original trigger

### Defense Layers

1. **Entry validation** - Reject invalid input at boundary
2. **Business logic** - Ensure data makes sense
3. **Environment guards** - Context-specific protection
4. **Debug instrumentation** - Capture forensics

## Red Flags - STOP

- "Quick fix for now"
- "Just try changing X"
- Proposing fixes before tracing data flow
- 3+ fixes failed → question architecture

## Process

1. **Start with systematic.md** on first error
2. **Use root-cause-tracing.md** if error is deep in stack
3. **Apply defense-in-depth.md** after finding root cause
