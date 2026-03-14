# Updating the /cc Skill

Process for syncing this skill with new Claude Code releases.

## When

After a new Claude Code release with extensibility changes (new hooks, commands, skill features, agent options, plugin system changes, settings).

## Process

1. **Fetch release notes** — `https://github.com/anthropics/claude-code/releases/tag/v{VERSION}`
2. **Read every reference file** in `references/` — understand current coverage before changing anything
3. **Compare line by line** — identify new features, changes, deprecations not reflected in references
4. **Update existing references** — add new content to the reference that owns that topic. Only create new files for genuinely new topics
5. **Gap analysis pass** — re-read release notes against updated references, fix anything missed
6. **Update version in Skill.md** — change the "Last synced" line at the top: `Last synced with Claude Code **v{VERSION}** ({DATE}).`

## Validation

After updates:

- Every new extensibility feature in the release notes has a home in a reference file
- No duplicate content across references
- Style matches existing: bullets not tables, direct language, no bloat
- Skill.md topics list is current (new references added, stale ones removed)
- "Last synced" version in Skill.md matches the release you just processed

## What to Cover

- New/changed hook events
- New/changed slash commands and CLI flags
- New/changed skill frontmatter fields
- New/changed agent frontmatter fields
- New/changed plugin system features
- New/changed settings.json options (extensibility-relevant only)
- Deprecations and removals
- Bug fixes that reveal previously undocumented behavior

## What to Skip

- Internal performance improvements (not configurable)
- Enterprise-admin-only settings (outside extensibility scope)
- UI/UX polish (not actionable for skill/hook/plugin authors)
