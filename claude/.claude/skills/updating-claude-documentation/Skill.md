---
name: updating-claude-documentation
description: Use when editing, creating, reviewing, or auditing Claude.md files and other Claude documentation - skills, agents, commands, hooks, settings (project .claude/ or personal ~/.claude/). Triggers: "update docs", "save to Claude.md", "document this", pre-commit prompts, contradictions with code, splitting bloated files, creating new Claude.md in undocumented directories.
---

# Updating Claude.md Documentation

Guide for maintaining project Claude.md files - capturing decisions, preserving context across sessions, preventing documentation drift, and proactively creating hierarchy when working in new areas.

## Triggers

1. **Explicit request** - "update docs", "save this to Claude.md", etc.
2. **Pre-commit prompt** - Before any commit, use AskUserQuestion: "Should we update docs for these changes?"
3. **Opportunistic flags** - When reading a Claude.md, use AskUserQuestion to flag:
   - Contradictions with actual code (staleness)
   - Files covering too many concerns (needs splitting into deeper hierarchy)
4. **Pattern detection** - Offer to document when noticing:
   - Repeated explanations across sessions
   - Emerging conventions that should be codified
   - User corrections to assumptions or behavior
5. **Missing hierarchy** - When working in a directory without a Claude.md but with established patterns, proactively offer to create one

## Naming Convention

Always use `Claude.md` (PascalCase) - never `CLAUDE.md` or `claude.md`. This follows the naming skill's "Never ALL_CAPS" rule.

**Proactive offers (impact 9-10 decisions):**
- Architectural changes
- New patterns introduced
- Breaking changes to existing conventions

## Update Process

**Prerequisites:**
- Target Claude.md file path
- Topic to document (one concept only)
- User's instruction or context

**Step 1: Research (Subagent)**

Dispatch subagent to read documentation hierarchy:
- Target Claude.md file
- All Claude.md files in parent directories
- All Claude.md files in child directories

Subagent returns summary:
- File structure (sections and what they cover)
- Existing documentation on this topic (quotes or "none found")
- Conflicts with proposed content (quote both sides)
- Recommended placement with reasoning

Main agent avoids reading Claude.md files directly and prefers working from subagent findings only.

**Step 2: Handle Conflicts**

If existing content found, present options:
- A) Replace existing with new version
- B) Merge both (draft combined version)
- C) Add to different section
- D) Cancel - existing is sufficient

Wait for user choice.

**Step 3: Propose Diff**

```
File: [path]
Section: [section name]

Remove:
> [exact text being removed, if any]

Add:
> [exact text being added]

Reasoning: [why this placement, how it fits]
```

Rules:
- One concept per change
- Match existing voice/style
- Minimal additions - no drive-by improvements
- Self-review before proposing - minimal, elegant, direct

**Step 4: Execute**

After explicit approval:
- Apply exact change from approved diff
- Bump version (minor for additions, major for structural changes)
- Update last-updated date

## References

- [hierarchy.md](hierarchy.md) - Hierarchy & placement principles for Claude.md files
- [style-guide.md](style-guide.md) - Style rules for writing Claude.md content
- `.claude/rules/` - Alternative to Claude.md; files auto-loaded (2.0.64+)
- [Claude Code Changelog](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md) - Check for new features affecting documentation
