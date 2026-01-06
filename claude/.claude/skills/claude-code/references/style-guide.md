# Style Guide for Claude.md Files

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

## Writing Docs AI Will Read

- **Explicit over implicit** - State rules directly; AI won't infer from examples
- **Firm, specific, declarative** - "Use X for Y" beats "potentially consider X"
- **Front-load critical info** - First lines of sections get highest weight
- **Structured data** - Markdown with bullets/annotated file trees to compact info per character. Avoid tables & gaudy ASCII displays that waste tokens.