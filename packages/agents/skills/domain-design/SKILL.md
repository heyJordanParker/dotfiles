---
name: domain-design
description: Build and sharpen a project's domain model. Use when the user wants to pin down domain terminology or a shared language, record an architectural decision, or when another skill needs to maintain the domain model.
---

# Domain Design

Actively build and sharpen the project's domain model as you design. This is the *active* discipline — challenging terms, inventing edge-case scenarios, and writing the shared language and decisions down the moment they crystallise. (Merely *reading* `Domain.md` for vocabulary is not this skill — that is a one-line habit any skill can do. This skill is for when you are changing the model, not just consuming it.)

## Where it lives

- `Domain.md` at the repo root — the shared language across every system in the project.
- `docs/architecture/decisions/` — the numbered decision records.

Create files lazily — only when you have something to write. If no `Domain.md` exists, create it when the first term is resolved. If no `docs/architecture/decisions/` exists, create it when the first decision is recorded.

## During the session

### Challenge against the shared language

When the user uses a term that conflicts with the existing language in `Domain.md`, call it out immediately. "Your `Domain.md` defines 'cancellation' as X, but you seem to mean Y — which is it?"

### Sharpen fuzzy language

When the user uses vague or overloaded terms, propose a precise canonical term. "You're saying 'account' — do you mean the Customer or the User? Those are different things."

### Discuss concrete scenarios

When domain relationships are being discussed, stress-test them with specific scenarios. Invent scenarios that probe edge cases and force the user to be precise about the boundaries between concepts.

### Cross-reference with code

When the user states how something works, check whether the code agrees. If you find a contradiction, surface it: "Your code cancels entire Orders, but you just said partial cancellation is possible — which is right?"

### Update Domain.md inline

When a term is resolved, update `Domain.md` right there. Don't batch these up — capture them as they happen. Use the format in [references/domain-format.md](references/domain-format.md).

`Domain.md` should be totally devoid of implementation details. Do not treat it as a spec, a scratch pad, or a store for implementation decisions. It is a shared language and nothing else.

### Record decisions sparingly

Only offer to record a decision when all three are true:

1. **Hard to reverse** — the cost of changing your mind later is meaningful
2. **Not self-evident from the code** — a future reader will wonder "why did they do it this way?"
3. **The result of a real trade-off** — there were genuine alternatives and you picked one for specific reasons

If any of the three is missing, skip it. Use the format in [references/decisions-format.md](references/decisions-format.md).
