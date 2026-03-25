---
description: Break complex scenarios into self-contained decision questions with 4+ options each
---

# /ask

For complex multi-decision scenarios. Each decision = one AskUserQuestion call.

## When to Use
- 3+ independent decisions needing approval
- Claude.md / docs overhauls
- Architecture spanning multiple concerns
- Anything too complex for a single plan review

## Question Format

Each question is self-contained — the user answers without reading any other document:

```
## Architecture

src/
├── auth/
│   ├── validate.ts*      <- adding phone validation
│   └── middleware.ts
├── api/
│   └── routes.ts*        <- will call new validator
└── tests/
    └── auth.test.ts*     <- new test cases

Legend: * = affected file, <- = bird's eye context

---

### Decision: [What we're deciding]

**Context:** [Why this matters — 1-2 sentences]

**Current state:**
`path/to/file.ts:L23-30`
[relevant code snippet]

**Proposed change:**
`path/to/file.ts`
[what it would look like after]

**Pros:** [bullets]
**Cons:** [bullets]
```

Then use AskUserQuestion with 4+ options:
- Label: Short name + "(Recommended, 85%)" if applicable
- Description: Tradeoff, implication

## Process

1. Identify all decision points
2. Clarify gaps early — edge cases, error handling, scope boundaries before architecture
3. Research each (codebase, docs) until ≥85% confident
3. Skip decisions where you're ≥85% confident — just decide
4. For each <85% decision: use AskUserQuestion with full format above
5. One question at a time
6. Embed full context IN the question (user shouldn't need to read anything else)
7. Collect all answers before continuing to next steps

## Example

```
## Architecture

src/auth/
├── validate.ts*          <- adding phone validation here
├── types.ts
└── index.ts*             <- re-export new function

---

### Decision: Where should phone validation live?

**Context:** Adding phone validation for 2FA. Currently only email validation exists.

**Current state:**
`src/auth/validate.ts:12-18`
```ts
const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function validateEmail(email: string): boolean {
  return emailPattern.test(email);
}
```

**Proposed change:**
`src/auth/validate.ts`
```ts
const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const phonePattern = /^\+?[1-9]\d{1,14}$/;

export function validateEmail(email: string): boolean {
  return emailPattern.test(email);
}

export function validatePhone(phone: string): boolean {
  return phonePattern.test(phone);
}
```

**Pros:**
- Matches existing pattern
- Single file for all input validation

**Cons:**
- File grows with each new validator
- Mixed concerns (email vs phone)
```

**Options (via AskUserQuestion):**
- "Separate validatePhone() (Recommended, 85%)" / "Matches existing pattern. Simple."
- "Combined validateContact() (60%)" / "One function, but mixed concerns."
- "Validation class (50%)" / "OOP pattern. More structure than needed."
- "Schema-based with zod (55%)" / "Type-safe. New dependency, learning curve."

## Rules

- Use `/naming` skill for all identifiers in code examples
- One question at a time — don't batch

## Anti-patterns

- Batching multiple decisions into one question
- Asking without researching first
- Binary options (yes/no) — always provide 4+
- Context that requires reading the full plan or other documents
- Asking about trivial decisions you should just make
