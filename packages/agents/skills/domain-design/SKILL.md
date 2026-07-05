---
name: domain-design
description: Build or sharpen a project's Domain.md and Decisions. Use when the Architect wants to pin down project language, record a Decision, or when another Skill needs to change Domain.md.
---

# Domain Design

One Process keeps the project's language and Decisions precise while Shaping or Execution changes them.

## 1. Use the right home

Project language lives in the repo-root `Domain.md`. Decisions live in `docs/architecture/decisions/`. Create either only when there is something to write.

## 2. Challenge language against `Domain.md`

IF a term conflicts with `Domain.md`:
### Ask which meaning survives before writing
Call out both meanings and ask the Architect which one survives.

IF a term is vague or overloaded:
### Propose the precise project word
Check `Domain.md` and the code, then propose the precise word that fits the project.

## 3. Test terms with concrete cases

Use concrete cases that probe edge cases and force precision about the boundaries between concepts.

## 4. Check code claims against code

When the Architect states how something works, read the code and surface any contradiction.

## 5. Update `Domain.md` when a term resolves

Write the term immediately at the repo root.

Template:
    **Term**:
    {One or two sentences defining what it IS, not what it does.}
    _Avoid_: {the words it replaces}

### One word wins
Put replaced words under `_Avoid_`. Project-specific terms go in `Domain.md`; general programming concepts stay out even when the project uses them heavily. Group terms under subheadings when natural clusters emerge.

### Keep implementation details out
`Domain.md` holds the project's language, not implementation decisions.

## 6. Propose Decisions, then record the Architect's choice

Propose a Decision only when all three hold: it is hard to reverse, it is not self-evident from the code, and it is the result of a real trade-off. The Architect makes the Decision, typically through /interview.

Template:
    docs/architecture/decisions/000N-<the-decision>.md

    # {The Decision}
    {One to three sentences: situation, choice, and WHY — plus measured scores when an experiment produced them.}
