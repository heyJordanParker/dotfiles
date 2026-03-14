---
name: cc
description: Use when working with Claude Code itself - skills, agents, hooks, settings, documentation. Covers building, testing, sharing skills, updating Claude.md files, and plugin distribution.
---

# Claude Code

Last synced with Claude Code **v2.1.76** (2026-03-14).

Guide for working with Claude Code's extensibility system.

## Principles

Apply to all topics below:

- **Context is finite** — every token loaded competes with reasoning. No "nice to have" context — everything loaded is necessary or harmful
- **Progressive disclosure** — load minimum, drill deeper when required. Three tiers: entry points (routing + principles) → topic files (one complete workflow) → deep dives (specs, examples, edge cases)
- **Split along tasks, not topics** — will different tasks need different parts? Split. Will every task need everything? Don't
- **One job per file** — focused files > fewer files
- **Trace actual flows** — follow how agents use skills to find gaps
- **The rule** — every instruction prevents a specific mistake. Can't name the mistake? Delete the instruction
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

Based on what you need, read the relevant reference:

- **Building agents:** [writing-agents.md](references/writing-agents.md) — creating custom agents, agent-only skills, subagent types
- **Building skills:** [building-skills.md](references/building-skills.md) — creating, editing, moving skills
- **Testing skills:** [testing-skills.md](references/testing-skills.md) — TDD for skills, pressure testing
- **Claude.md files:** [claude-md.md](references/claude-md.md) — template, structure, style guide, and update process for Claude.md files
- **Hooks:** [hooks.md](references/hooks.md) — creating event-driven automation
- **Rules:** [rules.md](references/rules.md) — modular project instructions via .claude/rules/
- **Plugins & marketplace:** [plugins-marketplace.md](references/plugins-marketplace.md) — distributing skills/hooks/commands via plugin system
- **User commands:** [user-commands.md](references/user-commands.md) — built-in slash commands
- **Updating this skill:** [updating-cc-skill.md](references/updating-cc-skill.md) — syncing with new Claude Code releases

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

### Style

- Use bullets, not tables. Tables waste tokens on formatting.

## Process

1. **Identify topic** from triggers above
2. **Read relevant reference** for detailed guidance
3. **Follow reference instructions** exactly
