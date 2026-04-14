---
name: cc
description: Use when working with Claude Code itself - skills, agents, hooks, settings, documentation. Covers building, testing, sharing skills, updating Claude.md files, and plugin distribution.
---

# Claude Code

Last synced with Claude Code **v2.1.87** (2026-03-29).

Guide for working with Claude Code's extensibility system.

## Principles

Apply to all topics below:

- **The rule** — instructions exist exclusively to correct default behavior. If Claude Code would already do it without being told, the instruction doesn't belong. This is the #1 litmus test for any line in any Claude Code configuration — Claude.md, skills, agents, hooks, rules. Can't name the default behavior it overrides? Delete it
- **Context is finite** — every token loaded biases the agent's output. More context doesn't mean better output — irrelevant content dilutes the signal and steers the agent toward wrong concerns. Everything loaded is necessary or harmful
- **Progressive disclosure** — load minimum, drill deeper when required. Three tiers: entry points (routing + principles) → topic files (one complete workflow) → deep dives (specs, examples, edge cases)
- **Split along tasks, not topics** — will different tasks need different parts? Split. Will every task need everything? Don't
- **One job per file** — focused files > fewer files
- **Trace actual flows** — follow how agents use skills to find gaps
- **Direct language** — "Use X" not "consider using X". Never: consider, might, should, could, maybe, perhaps
- **Signs of bloat** — decorative formatting, examples repeating what the rule said, process sections duplicated across files, tables instead of bullets

## Triggers

- "cc", "claude code", "claude-code"
- Creating/editing skills, agents, hooks, plugins
- Updating Claude.md documentation
- Context engineering and efficiency
- Testing skills with subagents
- Sharing skills upstream

## Topics

Read the reference that matches the problem you're solving:

- Teach agents a reusable process → [building-skills.md](references/building-skills.md)
- Verify a skill works under pressure → [testing-skills.md](references/testing-skills.md)
- Document project context for agents → [claude-md.md](references/claude-md.md)
- Automate reactions to events → [automating-with-hooks.md](references/automating-with-hooks.md)
- Create a specialized agent → [writing-agents.md](references/writing-agents.md)
- Share setup with other projects → [plugins-marketplace.md](references/plugins-marketplace.md)
- Find a built-in command → [user-commands.md](references/user-commands.md)
- Update this skill for a new release → [updating-cc-skill.md](references/updating-cc-skill.md)

## Quick Reference

### Skill Locations

- **Personal:** `~/.claude/skills/skill-name/`
- **Project:** `.claude/skills/skill-name/`
- **Plugin:** `plugin/skills/skill-name/`

### Agent-Only Skills

Skills with empty description frontmatter won't appear in available_skills context but can still be loaded by agents via the `skills:` frontmatter field. See [writing-agents.md](references/writing-agents.md) for the full pattern.

### File Naming

- `Skill.md` (PascalCase, not SKILL.md)
- `Claude.md` (PascalCase, not CLAUDE.md)

### Rules

Modular project instructions in `.claude/rules/`. Alternative to Claude.md for scoped guidance.

- **User:** `~/.claude/rules/` — all projects
- **Project:** `.claude/rules/` — current project (higher priority)
- Files discovered recursively. Symlinks supported
- `paths:` frontmatter scopes rules to file globs (e.g. `paths: **/*.ts` or a YAML list of globs). Without it, rules apply unconditionally
- Load order: user rules → project rules → Claude.md hierarchy (all coexist)
- One topic per file, descriptive filenames, subdirectories to organize

### Style

- Use bullets, not tables. Tables waste tokens on formatting.

## Process

1. **Identify topic** from triggers above
2. **Read relevant reference** for detailed guidance
3. **Follow reference instructions** exactly
