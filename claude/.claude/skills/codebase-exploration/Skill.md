---
name: codebase-exploration
description: Use when exploring a codebase to understand structure, find files, check dependencies, or see recent changes. Provides verified commands for effective exploration.
---

# Codebase Exploration

Commands for exploring and understanding codebases.

## Triggers

- "explore the codebase", "understand this project"
- "find files", "what files exist", "project structure"
- "check dependencies", "what packages", "what's installed"
- "recent changes", "what changed"

## Before

1. **Identify what you need** - Structure? Specific files? Dependencies? Recent changes?
2. **Check project type** - Node/Bun? PHP/Composer? Other?

## During

### Files

```bash
git ls-files                              # all tracked files
git ls-files '*.ts'                       # by pattern
git diff --name-only                      # changed (unstaged)
git diff --name-only --cached             # changed (staged)
git status --porcelain                    # working state
find . -name '*.ts' -not -path './node_modules/*'
```

### Structure

```bash
eza --tree -L 2 -I node_modules           # project tree
```

### Dependencies

**Node/Bun:**
```bash
jq -r '(.dependencies//{}),(.devDependencies//{}) | keys[]' package.json
bun pm ls                                 # installed packages
```

**PHP/Composer:**
```bash
jq -r '(.require//{}),(.["require-dev"]//{}) | keys[]' composer.json
composer show --direct                    # installed packages
```

### Recent Changes

```bash
git log --oneline -5                      # recent commits
git log --oneline --since="1 week ago"    # commits this week
git diff HEAD~5 --stat                    # what changed in last 5 commits
```

## After

- Found what you needed?
- Note patterns for future reference

## Related Skills

- `building-skills` - Uses exploration to understand existing skill patterns
- `show-architecture` - Visualizing codebase structure
