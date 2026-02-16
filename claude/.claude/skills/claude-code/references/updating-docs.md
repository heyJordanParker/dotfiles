
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

## Hierarchy & Placement

**How Claude.md Files Work:**

Claude.md files are hierarchical. When opening any file, Claude automatically reads every Claude.md in the file tree from root to that file's directory. This means:
- Parent context flows down automatically
- Don't repeat parent content in child files
- Each file only needs to add what's specific to its level

**Core Principles:**

- **Push context as deep as possible while keeping it discoverable.** Root stays navigable, details live where they're needed.
- **Write minimum documentation that provides full context.** AI context is limited and precious. Too little → agents can't complete tasks. Too much → agents overflow and forget critical details. Every line must earn its place. No fluff.

**What Goes Where:**

- **Project root:** Vision, principles, architecture overview, dev commands, navigation guide to the codebase
- **Feature/module/namespace directories:** What this area does, how it's organized, key patterns for working here, rules specific to this area
- **Code-heavy directories:** Tactical docs with code examples, function signatures, usage patterns
- **Organizational directories:** Structural docs explaining what's inside and how subdirectories relate

**The deciding factor is what's in the directory:**
- Mostly code files → include concrete examples and API patterns
- Mostly subdirectories → explain the structure and relationships

**Each Claude.md answers:** "What do I need to know to work effectively in this directory?"

**Signals to Split:**

- A section grows beyond ~10 bullet points
- Content primarily serves a subdirectory's developers
- Adding implementation details to a strategic document

**Signals to Create New Claude.md:**

- Working in a directory with established patterns but no Claude.md
- A parent file is covering concerns that belong deeper
- Developers in that directory would benefit from local context

**Placement Decision Process:**

1. Identify scope of what's being documented
2. Find closest existing Claude.md that matches scope
3. If no good match, consider creating new file at appropriate level
4. Propose placement with reasoning, wait for approval

## Style Guide

**File Metadata:**
- File name must be `Claude.md` (PascalCase, not ALL_CAPS)
- Version number at top (e.g., `v1.2`) - bump major for structural changes, minor for content additions
- Last updated date (e.g., `Updated: 2026-01-01`)

**Structure:**
- 2-space indentation
- Bullet points under descriptive headings
- `##` for main sections, `###` for subsections
- No orphan content (everything under a heading)
- Keep sections under ~10 bullet points (split if larger)

**Voice:**
- Imperative ("Use X" not "You should use X")
- No filler words or preamble
- No hedging ("consider", "might", "should", "could") - use direct commands ("Use", "Run", "Do")
- Examples illustrate, not prescribe - describe the capability, then give examples with "e.g." or "such as": "prettify html code (e.g. buttons, links)" not "prettify buttons and links"
- No fluff - every line earns its place

**Formatting:**
- **Bold** for key terms being defined
- `Backticks` for code, file paths, commands
- Numbered lists only for procedural steps
- Bullet points for everything else

**What to Exclude:**
- Session-specific context
- Temporary decisions
- Anything that needs frequent updates
- Content that belongs in a deeper file

**Writing Docs AI Will Read:**
- **Explicit over implicit** - State rules directly; AI won't infer from examples
- **Firm, specific, declarative** - "Use X for Y" beats "potentially consider X"
- **Front-load critical info** - First lines of sections get highest weight
- **Structured data** - Markdown with bullets/annotated file trees to compact info per character. Avoid tables & gaudy ASCII displays that waste tokens.

## References

- `.claude/rules/` - Alternative to Claude.md; files auto-loaded (2.0.64+)
- [Claude Code Changelog](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md) - Check for new features affecting documentation
