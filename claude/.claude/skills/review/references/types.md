# Type Design Review

Dispatch type-design-analyzer agent to audit type quality.

**Focus:** Invariants, encapsulation, impossible states, type safety.

## When to Use

- Introducing new types/interfaces
- Refactoring data models
- Domain modeling
- Before merging type-heavy changes

## How to Request

**1. Get changed files:**
```bash
git diff --name-only HEAD~1 -- '*.ts' '*.tsx'  # or appropriate pattern
```

**2. Dispatch type-design-analyzer:**

Use Task tool with `pr-review-toolkit:type-design-analyzer` subagent type:

```
Review type design in these changes:

Files: {CHANGED_FILES}
Git range: {BASE_SHA}..{HEAD_SHA}

Analyze:
- Invariant expression (does type make invalid states unrepresentable?)
- Encapsulation (are internals properly hidden?)
- Usefulness (does type add value vs primitive?)
- Enforcement (can invariants be violated?)
```

**3. Act on findings:**
- Tighten types to eliminate impossible states
- Add proper encapsulation for mutable state
- Replace primitives with domain types where valuable

## What It Catches

- **Weak invariants:** `status: string` vs `status: 'pending' | 'done'`
- **Exposed internals:** Public mutable arrays
- **Primitive obsession:** `userId: string` vs `UserId` brand
- **Impossible states:** `{ loading: boolean, data: T, error: Error }`

## Integration

Use after anti-slop check finds §3 (Type escapes) issues for deeper analysis.
