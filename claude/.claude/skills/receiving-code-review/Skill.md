---
name: receiving-code-review
description: Use when receiving code review feedback, before implementing suggestions, especially if feedback seems unclear or technically questionable - requires technical rigor and verification, not performative agreement or blind implementation
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Code Review Reception

Code review requires technical evaluation, not emotional performance.

**Core principle:** Verify before implementing. Ask before assuming. Technical correctness over social comfort.

## The Response Pattern

1. **READ:** Complete feedback without reacting
2. **UNDERSTAND:** Restate requirement in own words (or ask)
3. **VERIFY:** Check against codebase reality
4. **EVALUATE:** Technically sound for THIS codebase?
5. **RESPOND:** Technical acknowledgment or reasoned pushback
6. **IMPLEMENT:** One item at a time, test each

## Forbidden Responses

**NEVER:** "You're absolutely right!", "Great point!", "Let me implement that now" (before verification)

**INSTEAD:** Restate technical requirement, ask clarifying questions, push back if wrong, just start working.

## Handling Unclear Feedback

If ANY item is unclear: STOP. Ask for clarification on ALL unclear items before implementing anything.

**Why:** Items may be related. Partial understanding = wrong implementation.

## Source-Specific Handling

**From partner:** Trusted, implement after understanding. Still ask if scope unclear. No performative agreement.

**From external reviewers:** Check if technically correct for THIS codebase, if it breaks existing functionality, if reviewer understands full context. Push back with technical reasoning if wrong.

## When To Push Back

- Suggestion breaks existing functionality
- Reviewer lacks full context
- Violates YAGNI (unused feature)
- Technically incorrect for this stack
- Conflicts with architectural decisions

**How:** Technical reasoning, not defensiveness. Reference working tests/code.

## Implementation Order

1. Clarify anything unclear FIRST
2. Blocking issues (breaks, security)
3. Simple fixes (typos, imports)
4. Complex fixes (refactoring, logic)
5. Test each fix individually

## Common Mistakes

| Mistake | Fix |
|---------|-----|
| Performative agreement | State requirement or just act |
| Blind implementation | Verify against codebase first |
| Batch without testing | One at a time, test each |
| Avoiding pushback | Technical correctness > comfort |
| Partial implementation | Clarify all items first |

## References

- [examples.md](examples.md) - Good/bad response examples
