---
name: claude-code
description: Use when working with Claude Code itself - skills, agents, commands, hooks, settings, documentation. Alias "cc". Covers building, testing, sharing skills, updating Claude.md files, and context engineering.
---

# Claude Code

Guide for working with Claude Code's extensibility system.

## Triggers

- "cc", "claude code", "claude-code"
- Creating/editing skills, agents, commands, hooks
- Updating Claude.md documentation
- Context engineering and efficiency
- Testing skills with subagents
- Sharing skills upstream

## Topics

Based on what you need, read the relevant reference:

| Topic | Reference | When to use |
|-------|-----------|-------------|
| **Building skills** | [building-skills.md](references/building-skills.md) | Creating, editing, moving skills |
| **Testing skills** | [testing-skills.md](references/testing-skills.md) | TDD for skills, pressure testing |
| **Sharing skills** | [sharing-skills.md](references/sharing-skills.md) | Contributing skills upstream via PR |
| **Updating docs** | [updating-docs.md](references/updating-docs.md) | Editing Claude.md files |
| **Context engineering** | [context-engineering.md](references/context-engineering.md) | Writing efficient instructions |

## Quick Reference

### Skill Locations

| Type | Path |
|------|------|
| Personal | `~/.claude/skills/skill-name/` |
| Project | `.claude/skills/skill-name/` |
| Plugin | `plugin/skills/skill-name/` |

### Agent-Only Skills

Skills with empty description frontmatter won't appear in available_skills context but can still be loaded by agents via the `skills:` frontmatter field.

```yaml
# Agent frontmatter
skills: skill-name-1, skill-name-2
```

### File Naming

- `Skill.md` (PascalCase, not SKILL.md)
- `Claude.md` (PascalCase, not CLAUDE.md)

## Process

1. **Identify topic** from triggers above
2. **Read relevant reference** for detailed guidance
3. **Follow reference instructions** exactly
