---
description: Plan a feature using pragmatic engineering principles
---

Plan implementation using the 5-step algorithm. Question requirements, delete scope, simplify approach.

## Instructions

Before writing any code, run through the pragmatic engineering process:

### Step 0: Understand

Before planning, fully understand the request:

1. **Explore the codebase** - Find related code, patterns, dependencies
2. **Answer your own questions first** - Search code before asking user
3. **Ask remaining questions** - Use standalone questions with full context:
   - Include file paths, code examples, risk assessment
   - User answers without scrolling up
4. **Confirm understanding** - Summarize intent back to user

**Mistake this prevents:** Planning based on assumptions instead of code reality.

### Step 1: Question Requirements

Ask yourself and the user:

- Does this need to exist?
- Who asked for this? Can you question them?
- Is this solving a real problem or an imagined one?
- What happens if we don't build this?

**AI-generated requirements get extra scrutiny.** Pattern matching isn't enough.

### Step 2: Delete Scope

What can be removed from this feature?

- Which requirements are actually optional?
- What's the epicenter - what can't be removed?
- Can we solve 80% of the problem with 20% of the work?
- Are we building features "for later"?

**Target:** If you're not adding things back later, you didn't delete enough.

### Step 3: Simplify

For what remains:

- What's the simplest solution that works?
- Can we use existing libraries instead of building?
- Can a single developer understand the whole thing?
- Can any component be rewritten in 10 minutes?

**Never optimize something that shouldn't exist.**

### Step 4: Plan the Build

Only after steps 1-3:

- Interface first - start with user experience, work backward
- Convention over configuration - use strong defaults
- 2-day quality horizon - plan to be shippable within 2 days
- Loosely coupled - independent components, one-way dependencies

### Step 5: Validate with Skill

Use the `pragmatic-engineering` skill to validate your plan against:
- Red flags (building before understanding, abstractions "for later", etc.)
- The 5-step algorithm (did we actually delete enough?)
- Code philosophy (will this be trivial to maintain?)

## Asking Questions

Every question must be **standalone**:
- Include: file path, code example, why it matters
- User answers without reading earlier output
- Try answering via code search first

## Output Format

```
# Plan: [Feature Name]

## Requirements (after questioning)
- [Only requirements that survived scrutiny]

## Deleted (not building)
- [What we decided not to build and why]

## Approach (simplified)
- [The simplest approach that works]

## Implementation Steps
1. [Concrete step]
2. [Concrete step]
...

## Validation
- [ ] Can be shipped in 2 days
- [ ] Any component rewritable in 10 minutes
- [ ] No abstractions without 3+ use cases
- [ ] Using libraries for heavy lifting
```
