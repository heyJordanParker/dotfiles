# Rules

Modular project instructions in `.claude/rules/`. Alternative to single Claude.md file.

## Locations

- **User:** `~/.claude/rules/` — all projects
- **Project:** `.claude/rules/` — current project only (higher priority)

Files discovered recursively. Symlinks supported.

## Frontmatter

**`paths`** — scope rules to specific files:

```yaml
---
paths: **/*.ts
---
```

**Glob patterns:**
- `**/*.ts` — all TypeScript files
- `src/**/*` — everything under src/
- `src/**/*.{ts,tsx}` — braces for multiple extensions
- `{src,lib}/**/*.ts, tests/**/*.test.ts` — comma-separated patterns

**Without `paths`:** rules apply unconditionally to all files.

## Load Order

1. User rules (`~/.claude/rules/`)
2. Project rules (`.claude/rules/`) — higher priority
3. Claude.md hierarchy — separate but coexists

## Best Practices

- One topic per file (testing.md, api-design.md)
- Use subdirectories to organize (frontend/, backend/)
- Only use `paths:` when rules truly apply to specific files
- Descriptive filenames

## Example

```
.claude/rules/
├── frontend/
│   ├── react.md
│   └── styles.md
├── backend/
│   └── api.md
└── testing.md
```

## References

- [Official docs](https://code.claude.com/docs/en/memory)
