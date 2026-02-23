
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

**Write Files**

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

**Write Direct** - Name the failures each instruction prevents. Keep files under 100 lines — split or trim if longer. "Use X" not "consider using X."

**Structure References** - References serve processes, not topics.

- Name after what you're DOING: `building-skills.md` — not what the topic IS: `context-engineering.md`
- **Litmus test:** verb phrase = process. "building skills" ✓. "context engineering" ✗.
- If content is "generally useful knowledge":
  - Universal → entry point (Skill.md)
  - Process-specific → the process reference that uses it
  - Both → split between them

**Signs structure is wrong:**
- Agent always reads files A and B together → merge into one file
- Agent loads file, uses <10% → split along task boundaries
- Same info in multiple files → move to common parent
- Agent can't complete task without principles → principles not in entry point

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
3. Follow principles in Skill.md and "Write Direct" guidance above

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

**Required:**
- **name:** ≤64 chars, lowercase, hyphens, numbers — must match directory name
- **description:** ≤1024 chars — critical for auto-activation, be specific about triggers

**Common:**
- **allowed-tools:** Array of tool names — optional, restricts which tools Claude can use
- **context:** `fork` runs skill in forked sub-agent context
- **agent:** Specify agent type for execution (e.g., `agent: code-reviewer`)
- **user-invocable:** `false` hides from slash command menu (default: `true` for skills in `/skills/`)
- **hooks:** Define scoped PreToolUse/PostToolUse/Stop hooks (see hooks.md)

**Optional:**
- **compatibility:** ≤500 chars — environment requirements (intended product, system packages, network access)
- **license:** For open-source skills (e.g., `MIT`, `Apache-2.0`)
- **metadata:** Custom key-value pairs (author, version, mcp-server, tags)

**Security restrictions:**
- No XML angle brackets (`<` `>`) in frontmatter — it appears in system prompt, could inject instructions
- No "claude" or "anthropic" in skill names — reserved

Both syntaxes work for allowed-tools:
```yaml
# Array
allowed-tools:
  - Read
  - Grep

# Inline
allowed-tools: [Read, Grep]

# Tool-specific patterns
allowed-tools: "Bash(python:*) Bash(npm:*) WebFetch"
```

**Note**: If the skill requires external packages, list them in the description or compatibility field.

## Discovery

Claude auto-discovers skills from all three locations (personal, project, plugin). No explicit invocation needed.

**Activation**: Claude reads descriptions and decides when to use a skill based on the current task. Generic descriptions fail — be explicit about triggers and use cases.

**Phases:**
1. **Discovery**: Claude reads frontmatter to decide if skill is relevant
2. **Execution**: Claude loads supporting files only if skill activates

Comprehensive documentation doesn't bloat initial decision-making.

**Writing descriptions:**

```yaml
# Good — specific triggers, clear scope
description: Manages Linear project workflows including sprint planning,
  task creation, and status tracking. Use when user mentions "sprint",
  "Linear tasks", "project planning", or asks to "create tickets".

# Good — negative triggers prevent over-activation
description: Advanced data analysis for CSV files. Use for statistical
  modeling, regression, clustering. Do NOT use for simple data exploration
  (use data-viz skill instead).

# Bad — too vague, no triggers
description: Helps with projects.

# Bad — no trigger phrases
description: Creates sophisticated multi-page documentation systems.
```

**Debugging activation:**
- Under-triggering: add keywords, especially technical terms users would say
- Over-triggering: add negative triggers (`Do NOT use for...`), narrow scope
- Test: ask Claude "When would you use the [skill-name] skill?" — it quotes the description back, revealing gaps

**Hot reload**: Skills reload automatically when modified — no restart needed.

**Nested discovery**: Skills in nested `.claude/skills/` directories (within project subdirectories) are auto-discovered.

## Related Skills

- [claude-md.md](claude-md.md) - Template, structure, and update process for Claude.md files
- `codebase-exploration` skill - Commands for exploring codebases

## References

- [building-examples.md](building-examples.md) - Good/bad examples with rationale
