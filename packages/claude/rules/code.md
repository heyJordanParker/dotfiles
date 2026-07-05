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

### Comment only unobvious WHY
Write one-line comments only for a hidden constraint, a workaround for a specific bug, a non-obvious invariant, or a business Rule the reader cannot derive from the code.
Never: docblocks, `@param`/`@return`, `/** ... */`, JSDoc, PHPDoc, or comments starting with `This`, `Returns`, `Handles`, `Used`, `Checks if`, or `Gets the`.

### Show the mechanism behind every claimed property
When you claim a property — idempotent, decoupled, in-process, queued, atomic, thread-safe — the name is not the mechanism. Show the mechanism in the same sentence, in code and in chat.
Example: "Idempotent via a `$loaded` early-return" states both; "idempotent" alone is a placeholder the reader fills their own way.

### Inline until there are two concrete callers
Two concrete callers are required before any wrapper. One caller means inline it.
Never: optional parameters, config knobs, or extension points for hypothetical future use.

### Delete cleanly
Remove unused code without preserving old call sites or shapes as a capability.
Never: renaming unused vars to `_x`, re-exporting removed types, or leaving `// removed for X` comments.

### Validate by running the thing
Validation means it ran: the migration ran, the code executed, the tests passed, the User-facing Critical Path was exercised, with the Evidence shown. State a criterion as a concrete input and output — a command with its expected status and body — never "should work" or "observably correct". If a Critical Path genuinely cannot be exercised, say so plainly.

### Verify what changed and what the change reaches
Verify what you changed and what your change reaches. Do not re-verify unchanged code with no reason to be broken — the baseline works until Evidence shows otherwise.

IF refactoring, consolidating, or simplifying code:
### Reduce the code
Refactoring means reduction — fewer lines, fewer files, fewer abstractions. A refactor that produces more code failed.

IF refactoring, consolidating, or simplifying code:
### Delete duplicate code and use existing patterns
Delete duplicate code and use existing patterns.
Never: new wrapper functions or types "for clarity".

IF code looks unused, dead, half-wired, or unfinished:
### Treat "looks unused" as unproven
"Looks unused" is never "is unused." Usage hides in routes, config, dependency injection, reflection, serialization, external callers, Agents, and half-built features. You cannot prove code is dead by reading it, and counting references proves nothing — never quantify usage to justify the change.

IF code looks unused, dead, half-wired, or unfinished:
### Assume the code solved a real problem
Assume a competent author wrote it to solve a real problem under Context you may not have. Start from "why would someone write this?", never "this is junk" — assuming prior work was stupid is how you delete something load-bearing.

IF code looks unused, dead, half-wired, or unfinished:
### Recover the WHY before touching it
Recover the WHY: read the code, what it touches, and its git history and blame. State in one sentence the problem it was written to solve. If you cannot, you do not understand it well enough to touch it — keep digging or leave it.

IF code looks unused, dead, half-wired, or unfinished:
### Determine real status by reasoning
Determine whether the code is live but reached in a way static reading misses, staged for a feature still being built, or genuinely orphaned because its problem is gone.

IF code looks unused, dead, half-wired, or unfinished:
### Preserve the solved problem
Propose a course of action that still solves that same problem the best way for the situation now. Lay out several options with /pcc if they exist. Removal is valid only once you can name the problem and show it no longer exists.

IF code looks unused, dead, half-wired, or unfinished:
### Avoid dead-code failure modes
Avoid looks-unused-equals-unused, stupid-predecessor, and silent delete/rewrite.

IF adding error handling, try/catch blocks, or recovery logic in development:
### Fail fast in development
Every error stops with a clear message. Never catch silently.

IF adding error handling, try/catch blocks, or recovery logic in production:
### Log production errors and degrade only non-critical features
Log everything in production. Non-critical features degrade gracefully.

IF a workaround seems necessary:
### Name the Architecture fix being avoided
Describe the Architectural fix you are avoiding and why before proposing the workaround.

IF a workaround seems necessary:
### Get approval before temporary workarounds
Temporary workarounds require the Architect's approval and documented reasoning.

IF the file you are editing shows callers ≥ 20 or complexity at the repo p95, or the next action is destructive, hard to reverse, or visible outside the repo:
### Run /critical-path first
Run /critical-path before editing or taking the action.
