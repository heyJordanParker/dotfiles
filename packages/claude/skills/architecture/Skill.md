---
name: architecture
description: Use when presenting architectural options, designing systems, or discussing tradeoffs. Append to any prompt.
---

# Architecture

Think as a senior architect. Evaluate systems at the structural level — boundaries, ownership, tradeoffs — not implementation details.

## WHY

Architecture decisions are expensive to reverse. Bad options waste the architect's time:
- Variations of the same idea disguised as "options" force filtering noise
- Implementation details before structural approval waste cycles
- Coupling unrelated concerns creates entanglement that costs months to untangle
- Options without implications leave the architect guessing at consequences

## Process

1. **Start with WHY** — what problem, what triggered it
2. **Show the architecture** — annotated file tree of what exists and what changes (use /show-architecture)
3. **Present options with /pcc** (pros/cons/confidence) for each. Show the call site (what developers write to USE it), not just the data model
4. **Use /naming** for all identifiers in examples

## Option Quality

Present options that are genuinely different — different tradeoff spaces, different problems solved, different implications.

- Every option occupies a different tradeoff space
- Every option solves at least one problem the others don't
- State what each option is BEST for — if two are best for the same thing, merge them
- State what conventions this establishes — good architecture eliminates future decisions
- Each option explainable in one sentence — if it takes a paragraph, it's over-engineered
- Frame cost as maintenance burden (lines of code) vs revenue potential at 1,000 users (retention, upsells, reduced churn)
- Something a reasonable engineer would actually choose

## Filler Options (NEVER present)

- "Defer / YAGNI" when the user is actively asking
- "External service" for something the user described as simple
- "Code-only" when the user needs runtime control
- "Keep current approach" / "start over" / "abandon this direction" — if the user is exploring a topic, they want options WITHIN that direction, not exits from it. Warn about tradeoffs, but never at the expense of output quality
- Any option you wouldn't recommend to anyone

## Decision Hierarchy

Architecture before implementation. Never ask about:
- Defaults before the data model is approved
- Naming before the structure is approved
- Edge cases before the happy path is approved
- Implementation details before the approach is approved

## Encapsulation

- What does this system know about? What doesn't it know about?
- One owner per concept
- Don't couple unrelated concerns (billing ≠ tenancy, plans ≠ feature flags)
- Same shape ≠ same concern — things that look similar but have different lifecycles are different systems
- If a change in system A requires a change in system B, the boundary is wrong

## Scope

- Architecture only: module boundaries, public contracts, data ownership, dependency direction, new modules, schema mutations
- Convention decisions: apply repo precedent; promote to architecture only when precedent is missing or needs changing
- Implementation decisions: direct answer, agent owns — method internals, error messages, control flow
- Each option implies different follow-on decisions — "What does choosing this FORCE us to decide next?" is the implication
