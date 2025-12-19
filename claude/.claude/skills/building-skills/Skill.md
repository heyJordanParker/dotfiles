---
name: building-skills
description: Use when creating, editing, moving, or understanding Claude Code skills. Covers skill structure, locations, building process, and maintenance.
---

# Skills

Guide for creating and maintaining Claude Code skills.

## Triggers

- "create a skill", "build a skill", "make a skill for..."
- "edit this skill", "update the skill", "add to skill"
- "move skill to dotfiles", "make skill personal/project"
- "how do skills work", "where do skills live"

## Locations

| Type | Path | Use for |
|------|------|---------|
| Personal | `~/.claude/skills/skill-name/` | General skills across all projects |
| Project | `.claude/skills/skill-name/` | Codebase-specific skills |
| Plugin | `plugin/skills/skill-name/` | Bundled with plugins |

**Discovery:** Claude auto-discovers from all locations. Restart required after changes.

## Building Skills

### Principles

1. **Project first** - Check existing skills for style. Project conventions → ecosystem standards → these principles.
2. **Principles over rules** - "Follow project patterns" beats a 20-row lookup table.
3. **YAGNI** - Abstract after duplication, not before.
4. **Capture the "why"** - Examples need rationale. "Bad: X, Good: Y, Why: Z"
5. **Progressive disclosure** - Skill.md stays lean. Move details to reference files.
6. **Ground in real code** - Explore codebase for actual patterns.
7. **Check existing docs** - Claude.md files, README, linter configs may have conventions.
8. **Validate incrementally** - Get key decisions approved first.
9. **Definition of done** - Every skill needs a validation checklist.

### Process

1. **Understand Scope** - Use AskUserQuestion. One question at a time, prefer multiple choice.
2. **Research** - Read existing skills for patterns. Check Claude.md files.
3. **Decide Placement** - Personal (general) vs Project (codebase-specific). Infer from context.
4. **Key Decisions First** - Get approval on structure before writing.
5. **Write Files** - Use `context-engineering` skill for effective documentation.
6. **Self-Review** - Run checklist before presenting.

### Self-Review Checklist

- [ ] Description clear about when to use? (≤1024 chars)
- [ ] Principles (adaptable) not rules (brittle)?
- [ ] Skill.md lean? Details in reference files?
- [ ] Every example includes the why?
- [ ] Understandable without brainstorming context?
- [ ] Has validation checklist?

## Editing Skills

### Update Content

1. Read current skill files
2. Identify what to change
3. Use `context-engineering` skill for writing
4. Restart Claude Code to pick up changes

### Add References

Format in Skill.md:
```markdown
## References

- [reference.md](references/reference.md) - Description
- [examples.md](references/examples.md) - Description
```

### Rename/Move

1. Rename directory to new name
2. Update `name:` in frontmatter to match directory
3. Update any commands that reference the skill
4. Restart Claude Code

### Move Personal ↔ Project

- **To personal:** Move from `.claude/skills/` to `~/.claude/skills/`
- **To project:** Move from `~/.claude/skills/` to `.claude/skills/`

## References

- [reference.md](reference.md) - Technical specs (frontmatter, structure, discovery)
- [examples.md](examples.md) - Good/bad examples with rationale
