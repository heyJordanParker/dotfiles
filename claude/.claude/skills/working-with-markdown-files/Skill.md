---
name: working-with-markdown-files
description: Critical rules and best practices for working with markdown files.
---

# Working with Markdown Files

## Naming Convention

Markdown files are ALWAYS named in PascalCase.md.

| Incorrect | Correct |
|-----------|---------|
| `CLAUDE.md` | `Claude.md` |
| `SKILL.md` | `Skill.md` |
| `README.md` | `ReadMe.md` |

## Claude.md Hierarchy

Claude.md documentation files are hierarchical. Settings cascade from general to specific:

```
~/.claude/Claude.md           # Global (personal, all projects)
project/Claude.md             # Project root
project/.claude/Claude.md     # Project-specific
project/subdir/Claude.md      # Directory-specific
```

Claude Code automatically reads Claude.md files hierarchically from a folder and all parent folders when accessing any file in that folder.

## Related Skills

- `updating-claude-documentation` - editing, creating, reviewing Claude.md files
- `context-engineering` - writing effective Claude documentation
- `building-skills` - creating new skills
