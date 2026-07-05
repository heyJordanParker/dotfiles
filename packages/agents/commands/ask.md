---
description: Break complex scenarios into self-contained decision questions with 4+ options each
---

# /ask

Ask the Architect one self-contained Architecture Decision question at a time.

- This Command is for complex scenarios: 3+ independent Architecture Decisions, Prompt overhauls such as Claude.md work, or anything too complex for one Proposal Review.
- Each Architecture Decision gets one AskUserQuestion call.
- Each question carries enough Context for the Architect to answer without reading another Prompt.

1. Identify every Architecture Decision.
2. Research each one in the codebase and relevant Prompts until the real options are clear.
3. Remove tactical decisions the Agent owns.
4. Rank the remaining options by correctness, best first.
5. Ask one question at a time.
6. Put the full Context in the question.
7. Collect every answer before continuing.

Template:
  ## Architecture

  src/
  ├── auth/
  │   ├── validate.ts*      <- adding phone validation
  │   └── middleware.ts
  ├── api/
  │   └── routes.ts*        <- will call the validator
  └── tests/
      └── auth.test.ts*     <- new test cases

  Legend: * = affected file, <- = Context

  ---

  ### Decision: [what the Architect is deciding]

  Context: [WHY this matters in 1-2 sentences]

  Current state:
  `path/to/file.ts:L23-30`
  [relevant code snippet]

  Proposed change:
  `path/to/file.ts`
  [what it would look like after]

  Pros:
  - [pros]

  Cons:
  - [cons]

AskUserQuestion options:
- Label: short name + `(Recommended, 85%)` when one option is best.
- Description: the tradeoff and implication.
- Count: 4+ options.

Example:
  ## Architecture

  src/auth/
  ├── validate.ts*          <- adding phone validation here
  ├── types.ts
  └── index.ts*             <- re-export new function

  ---

  ### Decision: Where should phone validation live?

  Context: Adding phone validation for two-factor authentication. Email validation already lives here.

  Current state:
  `src/auth/validate.ts:12-18`
  ```ts
  const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

  export function validateEmail(email: string): boolean {
    return emailPattern.test(email);
  }
  ```

  Proposed change:
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

  Pros:
  - Matches the existing pattern.
  - Keeps input validation in one file.

  Cons:
  - The file grows with each validator.
  - Email and phone validation are separate concerns.

  AskUserQuestion options:
  - "Separate validatePhone() (Recommended, 85%)" / "Matches the existing pattern. Simple."
  - "Combined validateContact() (60%)" / "One function, but mixes concerns."
  - "Validation class (50%)" / "More code than this needs."
  - "Schema-based with zod (55%)" / "Type-safe, but adopts a dependency."

### Use `/naming` for identifiers in code examples

Names must come from the codebase or the Architect's words.

### Ask one question at a time

Never batch multiple Architecture Decisions into one AskUserQuestion call.

### Research before asking

Never ask about a decision the code can settle.

### Use options, not yes/no

Never send a binary option. Provide 4+ ranked options.

### Make the question self-contained

Never depend on the Architect reading a Plan, Proposal, Prompt, file path, or line number outside the question.

### Do not ask about tactical decisions

If the Agent owns the decision, make it and keep going.

### Do not ask motivation probes

Never ask the Architect to explain why they want something so the Agent can choose.
