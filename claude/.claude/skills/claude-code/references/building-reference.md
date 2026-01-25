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

- **Personal:** `~/.claude/skills/skill-name/` — General skills across all projects
- **Project:** `.claude/skills/skill-name/` — Codebase-specific skills

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

- **name:** ≤64 chars, lowercase, hyphens, numbers — must match directory name
- **description:** ≤1024 chars — critical for auto-activation, be specific about triggers
- **allowed-tools:** Array of tool names — optional, restricts which tools Claude can use
- **context:** `fork` runs skill in forked sub-agent context
- **agent:** Specify agent type for execution (e.g., `agent: code-reviewer`)
- **user-invocable:** `false` hides from slash command menu (default: `true` for skills in `/skills/`)
- **hooks:** Define scoped PreToolUse/PostToolUse/Stop hooks (see hooks.md)

Both syntaxes work for allowed-tools:
```yaml
# Array
allowed-tools:
  - Read
  - Grep

# Inline
allowed-tools: [Read, Grep]
```

**Note**: If the skill requires external packages, list them in the description.

## Discovery

Claude auto-discovers skills from all three locations (personal, project, plugin). No explicit invocation needed.

**Activation**: Claude reads descriptions and decides when to use a skill based on the current task. Generic descriptions fail - be explicit about triggers and use cases.

## Progressive Disclosure

1. **Phase 1 (Discovery)**: Claude reads frontmatter to decide if skill is relevant
2. **Phase 2 (Execution)**: Claude loads supporting files only if skill activates

This means comprehensive documentation doesn't bloat initial decision-making.

**Hot reload**: Skills reload automatically when modified — no restart needed.

**Nested discovery**: Skills in nested `.claude/skills/` directories (within project subdirectories) are auto-discovered.
