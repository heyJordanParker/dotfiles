---
name: asking-decisions
description: Use when presenting complex multi-decision scenarios - structures each decision as a self-contained AskUserQuestion with architecture tree, code examples, and 4+ options
---

# Asking Decisions

Structure complex decisions as self-contained questions. Each question = mini architecture decision record.

## Core Principles

1. **Self-contained** - User answers without reading any other document
2. **4+ options minimum** - Maximize AskUserQuestion utility
3. **Research first** - Only ask if <85% confident after checking codebase/docs
4. **One at a time** - Don't batch questions
5. **Use naming skill** - All identifiers in examples must follow naming conventions

## Question Format

Each question includes these sections in the question text:

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

**Context:** [Why this matters, what triggered it - 1-2 sentences]

**Current state:**
`path/to/file.ts:L23-30`
```code
[relevant code snippet]
```

**Proposed change:**
`path/to/file.ts`
```code
[what it would look like after]
```

**Risk:** [What could go wrong - 1 sentence]
```

Then use AskUserQuestion tool with 4+ options:
- Label: Short name + "(Recommended, 85%)" if applicable
- Description: Tradeoff, implication

## Example

```
## Architecture

src/auth/
├── validate.ts*          <- adding phone validation here
├── types.ts
└── index.ts*             <- re-export new function

---

### Decision: Where should phone validation live?

**Context:** Adding phone validation for 2FA. Currently only email validation exists. Need to decide placement for maintainability.

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

**Risk:** Adding more exports increases surface area. Could grow into god-file.
```

**Options (via AskUserQuestion):**
- Label: "Separate validatePhone() (Recommended, 85%)" / Description: "Matches existing pattern. Simple."
- Label: "Combined validateContact() (60%)" / Description: "One function, but mixed concerns."
- Label: "Validation class (50%)" / Description: "OOP pattern. More structure than needed."
- Label: "Schema-based with zod (55%)" / Description: "Type-safe. New dependency, learning curve."

## Process

1. Identify all decision points in the work
2. Filter to high-impact (skip trivial choices you can make at ≥85% confidence)
3. Research each remaining decision until confident or blocked
4. For each <85% decision: present via AskUserQuestion with full format above
5. Wait for answer before next question
6. Collect all answers before proceeding with implementation

## Anti-patterns

- Batching multiple decisions into one question
- Asking without researching first
- Binary options (yes/no) - always provide 4+
- Context that requires reading the full plan
- Asking about trivial decisions you should just make
- Using ALL_CAPS in code examples
