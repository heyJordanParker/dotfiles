
# Building Skills

Guide for creating and maintaining Claude Code skills.

## Triggers

- "create a skill", "build a skill", "make a skill for..."
- "edit this skill", "update the skill", "add to skill"
- "move skill to dotfiles", "make skill personal/project"
- "how do skills work", "where do skills live"

## Principles

1. **Project first** - Check existing skills for style. Project conventions → ecosystem standards → these principles.
2. **Principles over rules** - "Follow project patterns" beats a 20-row lookup table.
3. **YAGNI** - Abstract after duplication, not before.
4. **Capture the "why"** - Examples need rationale. "Bad: X, Good: Y, Why: Z"
5. **Prefer examples to prose** — When defining acceptable patterns, use good/bad code examples instead of prose descriptions. The example shows what to do; the reasoning explains why it matters.
6. **Progressive disclosure** - Skill.md stays lean and routes to reference files. Group content by what gets used together — when the agent opens a file, most of it should be relevant.
7. **Ground in real code** - Explore codebase for actual patterns.
8. **Check existing docs** - Claude.md files, README, linter configs may have conventions.
9. **Validate incrementally** - Get key decisions approved first.
10. **Definition of done** - Every skill needs a validation checklist.

## Locations

- **Personal:** `~/.claude/skills/skill-name/` — General skills across all projects
- **Project:** `.claude/skills/skill-name/` — Codebase-specific skills
- **Plugin:** `plugin/skills/skill-name/` — Bundled with plugins

## Before

1. **Understand Scope** - Use AskUserQuestion. One question at a time, prefer multiple choice.
2. **Research** - Read existing skills for patterns. Check Claude.md files.
3. **Explore Related Skills** - Find skills that overlap or complement. Prevent duplicates. Plan cross-references.
4. **Decide Placement** - Personal (general) vs Project (codebase-specific). Infer from context.
5. **Key Decisions First** - Get approval on structure before writing.

## During

**Write Files** - Use `context-engineering` skill for effective documentation.

Structure skill execution in phases:
1. **Before** - What does the skill need to prepare? Prerequisites, research, setup.
2. **During** - The actual work. Steps, decisions, guidance.
3. **After** - Validation. Checklists, verification, cleanup.

**Avoid Stale Content** - You love hardcoding file lists. Don't. They go stale immediately.

- Mention only critical components by name
- Instruct the skill to use `codebase-exploration` to derive lists dynamically
- Never enumerate files, dependencies, or structure inline

**Bad:** "This project has: src/auth.ts, src/api.ts, src/utils.ts"

**Good:** "Run `git ls-files 'src/*.ts'` to see current source files"

## After

- [ ] Description clear about when to use? (≤1024 chars)
- [ ] Principles (adaptable) not rules (brittle)?
- [ ] Skill.md lean? Details in reference files?
- [ ] Every example includes the why?
- [ ] Understandable without prior context?
- [ ] Has validation checklist?
- [ ] Related skills identified and cross-referenced?

## Editing Skills

### Update Content

1. Read current skill files
2. Identify what to change
3. Use `context-engineering` skill for writing

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

## File Structure

```
skill-name/
├── Skill.md      # Required - must be named exactly "Skill.md"
├── reference.md  # Optional - lookup/technical specs
├── examples.md   # Optional - good/bad examples
├── scripts/      # Optional - utility scripts
└── templates/    # Optional - templates
```

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

**Phases:**
1. **Discovery**: Claude reads frontmatter to decide if skill is relevant
2. **Execution**: Claude loads supporting files only if skill activates

Comprehensive documentation doesn't bloat initial decision-making.

**Hot reload**: Skills reload automatically when modified — no restart needed.

**Nested discovery**: Skills in nested `.claude/skills/` directories (within project subdirectories) are auto-discovered.

## Related Skills

- [context-engineering.md](context-engineering.md) - Writing effective Claude documentation
- [updating-docs.md](updating-docs.md) - Editing Claude.md files
- `codebase-exploration` skill - Commands for exploring codebases

## References

- [building-examples.md](building-examples.md) - Good/bad examples with rationale
