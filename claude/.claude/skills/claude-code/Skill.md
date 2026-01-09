---
name: claude-code
description: Use when working with Claude Code itself - skills, agents, commands, hooks, settings, documentation. Alias "cc". Covers building, testing, sharing skills, updating Claude.md files, and context engineering.
---

# Claude Code

Guide for working with Claude Code's extensibility system.

## Principles

Apply to all topics below:

- **Context is finite** — every token loaded competes with reasoning
- **Progressive disclosure** — entry points route, details in references
- **One job per file** — focused files > fewer files
- **Trace actual flows** — follow how agents use skills to find gaps

## Triggers

- "cc", "claude code", "claude-code"
- Creating/editing skills, agents, commands, hooks
- Updating Claude.md documentation
- Context engineering and efficiency
- Testing skills with subagents
- Sharing skills upstream

## Topics

Based on what you need, read the relevant reference:

- **Building skills:** [building-skills.md](references/building-skills.md) — creating, editing, moving skills
- **Testing skills:** [testing-skills.md](references/testing-skills.md) — TDD for skills, pressure testing
- **Sharing skills:** [sharing-skills.md](references/sharing-skills.md) — contributing skills upstream via PR
- **Updating docs:** [updating-docs.md](references/updating-docs.md) — editing Claude.md files
- **Context engineering:** [context-engineering.md](references/context-engineering.md) — read when creating new skills/docs, restructuring, or debugging agent behavior
- **Hooks:** [hooks.md](references/hooks.md) — creating event-driven automation
- **Rules:** [rules.md](references/rules.md) — modular project instructions via .claude/rules/

## Quick Reference

### Skill Locations

- **Personal:** `~/.claude/skills/skill-name/`
- **Project:** `.claude/skills/skill-name/`
- **Plugin:** `plugin/skills/skill-name/`

### Agent-Only Skills

Skills with empty description frontmatter won't appear in available_skills context but can still be loaded by agents via the `skills:` frontmatter field.

```yaml
# Agent frontmatter
skills: skill-name-1, skill-name-2
```

### File Naming

- `Skill.md` (PascalCase, not SKILL.md)
- `Claude.md` (PascalCase, not CLAUDE.md)

### Style

- Use bullets, not tables. Tables waste tokens on formatting.

## Process

1. **Identify topic** from triggers above
2. **Read relevant reference** for detailed guidance
3. **Follow reference instructions** exactly
