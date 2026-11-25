---
paths:
  - "**/*.php"
  - "**/*.ts"
  - "**/*.tsx"
  - "**/*.js"
  - "**/*.jsx"
  - "**/*.py"
  - "**/*.rb"
  - "**/*.go"
  - "**/*.rs"
  - "**/*.java"
  - "**/*.c"
  - "**/*.cpp"
  - "**/*.h"
  - "**/*.hpp"
  - "**/*.cs"
  - "**/*.swift"
  - "**/*.kt"
  - "**/*.sh"
  - "**/*.sql"
  - "**/*.css"
  - "**/*.scss"
  - "**/*.vue"
---

### Write no comments
Write code that doesn't need comments or special cases. Use deterministic lints and tests for everything important or unusual.

### Support no legacy code
Production code serves only the current shape. Move old data with a one-time
migration, then delete the old path.
Never: a shim for an old field or state, a re-export of a removed type, an
unused variable renamed to `_x`, or a `// removed for X` comment.

### Verify by running what changed and what it reaches
Run the migration, the code, the tests, and the User-facing Critical Path
your change reaches. The baseline works until Evidence shows otherwise.

IF adding error handling, try/catch blocks, or recovery logic in development:
### Fail fast in development
Every error stops with a clear message. Never catch silently.

IF adding error handling, try/catch blocks, or recovery logic in production:
### Log production errors and degrade only non-critical features
Log everything in production. Non-critical features degrade gracefully.

IF the file you are changing shows `imported_by` of 20 or more or a `complexity_rank` of
`critical`, or the next action is destructive, hard to reverse, or visible outside the repo:
### Run /critical-path first
Run /critical-path before editing or taking the action.
