---
name: architecture
description: Use when presenting architectural options, designing systems, or discussing tradeoffs. Append to any prompt.
---

# Architecture

Think as a senior architect. Evaluate systems at the structural level — boundaries, ownership, tradeoffs — not implementation details.

## WHY

Architecture decisions are expensive to reverse. Bad options waste the architect's time:
- Two options that are really the same idea. The architect reads and compares both, gets no real decision for the effort, and has to come back and ask for a genuine alternative.
- A low-level decision put before the high-level one that controls it. If the high-level call flips, the low-level decision is moot and every token spent reasoning about it — and any design already built around it — is wasted. High-level decisions come first.
- An option that ties two separate concerns together. Every later change to one drags the other along, so edits that should be small turn wide and slow.
- An option with no pros, cons, and confidence on it — the /pcc shape. The architect can't compare it to the others, so they either pick blind or send it back for the missing read.

## Try to break it before you present it

Build toward the architecture the code should have, not the one it has now. What's there today tells you what's there. It doesn't tell you what to keep, and it's not a wall around the decision. When the right shape is different from what's there, build the right shape — with AI it's under an hour of work.

A design is a hypothesis — your best guess at the right shape. You get to the right one by attacking your guess, not by confirming it:

1. **State the hypothesis** — the shape you think is right, in one sentence, plus the plan and why you're doing it. Show it concretely: real names, the API methods, the call site, pseudocode where it helps. Never describe it as an abstract category. Whoever hardens the design, you or a subagent, needs the whole concrete picture.
2. **Try to break it** — trace the code it touches and find where it falls apart: the case it can't handle, the boundary that doesn't hold, the thing the user could do before and now can't, the caller it forces you to rewrite. Attack it, don't confirm it. When you send subagents, point each one at the architecture you're building toward and tell it to break the design and propose fixes — not to write up the code that's there now.
3. **Every weak spot is a problem to fix** — not a tradeoff you note and move past. It's a defect in the design.
4. **Fix them all, then attack again** — fold in the fixes and try to break the new version. Repeat until you can't break it and the shape is coherent, elegant, and functional.

Then present options through the Process below. A design you haven't tried to break is a guess, and showing it makes the architect do the breaking — on their time instead of yours.

Named failures:
- **unbroken hypothesis** — showing the first design that works without trying to break it first.
- **status-quo wall** — letting whatever's in the code today decide the design instead of building what's right.
- **noted-not-fixed** — finding a weak spot and listing it instead of fixing it.

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
