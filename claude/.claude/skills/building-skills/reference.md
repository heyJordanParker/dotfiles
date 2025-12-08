# Technical Reference

## File Structure

```
skill-name/
├── Skill.md      # Required - must be named exactly "Skill.md"
├── reference.md  # Optional - lookup/technical specs
├── examples.md   # Optional - good/bad examples
├── scripts/      # Optional - utility scripts
└── templates/    # Optional - templates
```

## Locations

| Type | Path | Use for |
|------|------|---------|
| Personal | `~/.claude/skills/skill-name/` | General skills across all projects |
| Project | `.claude/skills/skill-name/` | Codebase-specific skills |

## Frontmatter Fields

```yaml
---
name: skill-name
description: When to use this skill (≤1024 chars)
allowed-tools:  # Optional - restricts available tools
  - Read
  - Grep
  - Glob
---
```

| Field | Constraints | Notes |
|-------|-------------|-------|
| `name` | ≤64 chars, lowercase, hyphens, numbers | Must match directory name |
| `description` | ≤1024 chars | Critical for auto-activation. Be specific about triggers. |
| `allowed-tools` | Array of tool names | Optional. Restricts which tools Claude can use. |

**Note**: If the skill requires external packages, list them in the description.

## Discovery

Claude auto-discovers skills from all three locations (personal, project, plugin). No explicit invocation needed.

**Activation**: Claude reads descriptions and decides when to use a skill based on the current task. Generic descriptions fail - be explicit about triggers and use cases.

## Progressive Disclosure

1. **Phase 1 (Discovery)**: Claude reads frontmatter to decide if skill is relevant
2. **Phase 2 (Execution)**: Claude loads supporting files only if skill activates

This means comprehensive documentation doesn't bloat initial decision-making.

**Restart**: Changes to skills require Claude Code restart to take effect.
