---
name: systematic-debugging
description: Use when encountering any bug, test failure, or unexpected behavior - IMMEDIATELY enter on first error. Four-phase framework (basic checks, root cause, pattern analysis, hypothesis testing, implementation) that ensures understanding before attempting solutions
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Systematic Debugging

Random fixes waste time and create new bugs. Quick patches mask underlying issues.

**Core principle:** ALWAYS find root cause before attempting fixes. Symptom fixes are failure.

## The Iron Law

```
NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST
```

If you haven't completed Phase 1, you cannot propose fixes.

## When to Use

**Any technical issue:** Test failures, bugs, unexpected behavior, performance problems, build failures.

**Especially when:** Under time pressure, "quick fix" seems obvious, you've already tried multiple fixes, you don't fully understand the issue.

## The Five Phases

Complete each phase before proceeding to the next.

| Phase | Goal | Key Activities |
|-------|------|----------------|
| **0. Basic Checks** | Rule out obvious | Typos, wrong names, wrong values, copy-paste errors |
| **1. Root Cause** | Understand WHAT and WHY | Read errors, reproduce, check changes, gather evidence |
| **2. Pattern** | Find working reference | Find similar working code, compare differences |
| **3. Hypothesis** | Test theory | Form single hypothesis, test minimally |
| **4. Implementation** | Fix correctly | Create failing test, single fix, verify |

## Phase 0: Basic Checks First

Before complex theories, rule out the obvious:
1. Typos in strings, option names, variable names
2. Wrong values (empty, null, wrong type)
3. Copy-paste errors (forgot to change a name)
4. Simple logic errors (wrong operator, off-by-one)

If basic check finds issue → fix it. Only proceed to Phase 1 if basics ruled out.

## Phase 1: Root Cause Investigation

Before ANY fix attempt:
1. Read error messages carefully (complete stack traces)
2. Reproduce consistently (exact steps)
3. Check recent changes (git diff, new deps)
4. Gather evidence in multi-component systems (log at boundaries)
5. Trace data flow (use root-cause-tracing skill)

## Phase 2: Pattern Analysis

1. Find working examples in same codebase
2. Read reference implementations COMPLETELY (don't skim)
3. Identify ALL differences
4. Understand dependencies

## Phase 3: Hypothesis and Testing

1. Form single clear hypothesis: "I think X because Y"
2. Test with SMALLEST possible change
3. Did it work? → Phase 4. Didn't work? → New hypothesis
4. Don't add more fixes on top

## Phase 4: Implementation

1. Create failing test case (use test-driven-development)
2. Implement single fix (ONE change, no bundled refactoring)
3. Verify fix

**If 3+ fixes failed:** STOP. Question the architecture. Discuss before attempting more.

## Red Flags - STOP

- "Quick fix for now, investigate later"
- "Just try changing X"
- Proposing solutions before tracing data flow
- "One more fix attempt" (when already tried 2+)
- Each fix reveals new problem in different place

**ALL mean:** Return to Phase 1. **If 3+ fixes failed:** Question architecture.

## Common Rationalizations

| Excuse | Reality |
|--------|---------|
| "Issue is simple" | Simple issues have root causes too |
| "Emergency, no time" | Systematic is FASTER than thrashing |
| "I see the problem" | Symptoms ≠ root cause |
| "One more fix attempt" | 3+ failures = architectural problem |

## References

- [phases.md](phases.md) - Detailed phase instructions with examples
- [CreationLog.md](CreationLog.md) - How this skill was developed
- test-pressure-*.md - Test scenarios used to validate this skill
