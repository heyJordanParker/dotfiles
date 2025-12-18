---
name: building-skills
description: Use when explicitly asked to create a Claude Code skill. Guides the process from understanding scope through writing and validating the skill files.
---

# Building Skills

Guide for creating Claude Code skills that are grounded, lean, and effective.

## Triggers

Explicit request only: "create a skill", "build a skill", "make a skill for..."

## Principles

1. **Project first** - Check existing skills for style. Project conventions → ecosystem standards → these principles.

2. **Principles over rules** - "Follow project patterns" beats a 20-row lookup table. If you're writing exhaustive mappings, step back and find the underlying principle.

3. **YAGNI** - Abstract after duplication, not before. Don't build for hypothetical future needs.

4. **Capture the "why"** - Examples need rationale. "Bad: X, Good: Y" doesn't teach. "Bad: X, Good: Y, Why: Z" does.

5. **Progressive disclosure** - Skill.md stays lean. Move lookup tables and extensive examples to reference files.

6. **Ground in real code** - Explore the user's codebase for actual patterns. Principles without real context become theoretical.

7. **Check existing docs** - Look at Claude.md files, README, style guides, linter configs. The user may already have conventions documented.

8. **Validate incrementally** - Get key decisions approved first. Then ask if user wants section-by-section review.

9. **Definition of done** - Every skill needs a checklist to verify work is complete. Not process steps - output validation.

## Process

### 1. Understand Scope

Brainstorm recommended. Use AskUserQuestion:
- One question at a time
- Prefer multiple choice when possible
- Use concrete scenarios to probe preferences ("which would you pick?")

### 2. Research

- Read existing skills in `~/.claude/skills/` and `.claude/skills/` for style patterns
- Check Claude.md files, README, linter configs for relevant conventions
- Explore codebase for real examples that ground the principles

### 3. Decide Placement

- **Personal** (`~/.claude/skills/skill-name/`) - General skills that apply across projects
- **Project** (`.claude/skills/skill-name/`) - Codebase-specific skills

Infer from context. Ask only if confidence <90%.

### 4. Key Decisions First

Get approval on:
- Core principles/structure
- What belongs in Skill.md vs reference files

Then ask: "Want to review section-by-section, or should I write the full draft?"

### 5. Write Files

1. **Skill.md** - Lean core: principles + process
2. **reference.md** - Technical specs, lookup tables (if needed)
3. **examples.md** - Good/bad examples with rationale (if needed)

### 6. Self-Review

Before presenting:
- [ ] Is description clear about when to use? (≤1024 chars)
- [ ] Are these principles (adaptable) or rules (brittle)?
- [ ] Is Skill.md lean enough? Move details to reference files.
- [ ] Does every example include the why?
- [ ] Would someone else understand without the brainstorming context?
- [ ] Does skill have validation checklist? (what to verify when done)

## References

- [reference.md](reference.md) - Technical specs for skill files
- [examples.md](examples.md) - Good/bad examples with rationale
