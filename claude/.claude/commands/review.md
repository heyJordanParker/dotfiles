---
description: Code review gate - anti-slop, architecture, or request review
argument-hint: [anti-slop|architecture|request]
---

Use the `review` skill to run code review.

Review type: $ARGUMENTS

If no type specified, default to `anti-slop` for pre-commit review.

Types:
- **anti-slop** - Before commit: catch AI slop, YAGNI, dead code
- **architecture** - Check SOLID, encapsulation, dependency direction
- **request** - Dispatch code-reviewer subagent
