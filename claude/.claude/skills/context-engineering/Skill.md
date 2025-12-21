---
name: context-engineering
description: Use when writing or reviewing Claude documentation (Claude.md, skills, commands, agents). Focuses on context efficiency, clarity, and preventing AI mistakes through better instructions.
---

# Context Engineering

Write instructions that make the most of limited AI context.

**Core principle:** Information lives where it's needed. Not too much, not too little.

## The Rule

Every instruction prevents a specific mistake. If you can't name the mistake, delete the instruction.

## Context Hierarchy

Claude Code reads Claude.md files recursively from the current file's directory upward.

**Place information close to where it's used:**
- Root `Claude.md` → project-wide rules only
- `backend/Claude.md` → backend patterns
- `backend/Integrations/Claude.md` → integration-specific details

**Never:**
- Put everything in root Claude.md
- Duplicate information across hierarchy levels

## Writing Documentation

1. **One concept per change** - Pick ONE thing to document
2. **Find the right level** - Closest Claude.md to where it's used
3. **Check for duplicates** - Read parent/sibling Claude.md files first
4. **Write minimally** - Fewest words that prevent the mistake
5. **Self-review** - Cut anything that doesn't prevent a specific error

## Writing Skills/Commands/Agents

1. **Name the failures** - What mistakes will this prevent?
2. **Read similar files** - Match existing patterns exactly
3. **Write direct instructions** - "Use X" not "consider using X"
4. **Keep under 100 lines** - If longer, split or trim
5. **Self-review** - Remove hedging, redundancy, decoration
6. **Include definition of done** - Add validation checklist: what must be true when finished

## Language Rules

**Use:** "Use", "Run", "Do", "Add", "Delete", "Check"

**Never use:** "consider", "might", "should", "could", "maybe", "perhaps"

**Bad:** "You should consider checking for null values"
**Good:** "Check for null values"

## Context Efficiency

**Too little context:** Agent can't complete the task - missing critical info

**Too much context:** Agent forgets details - overflowed memory

**Right amount:** Agent completes task correctly on first try

**Signs of bloat:**
- Decorative formatting (boxes, fancy headers)
- Examples that repeat what the rule already said
- Process sections duplicated across files
- Tables with low information density

## Review Checklist

When reviewing Claude documentation:

- [ ] Information at the right hierarchy level?
- [ ] No duplication with parent/sibling docs?
- [ ] Direct language (no hedging)?
- [ ] Under 100 lines?
- [ ] Each instruction prevents a named mistake?
- [ ] Follows existing patterns in the project?
- [ ] Has definition of done checklist?

## Avoiding Stale Content

Don't hardcode lists that change as the project evolves. Derive from source instead.

**Instead of hardcoding:**
- File lists → `git ls-files` or glob patterns
- Dependencies → parse package.json/composer.json
- Project structure → `eza --tree -L 2`
- Recent changes → `git log --oneline -5`

**Example - bad:**
"This project has: src/auth.ts, src/api.ts, src/utils.ts"

**Example - good:**
"Run `git ls-files 'src/*.ts'` to see current source files"

### Verified Commands

**Files:**
```bash
git ls-files                              # all tracked files
git ls-files '*.ts'                       # by pattern
git diff --name-only                      # changed (unstaged)
git diff --name-only --cached             # changed (staged)
git status --porcelain                    # working state
find . -name '*.ts' -not -path './node_modules/*'
```

**Structure:**
```bash
eza --tree -L 2 -I node_modules           # project tree
```

**Dependencies:**
```bash
jq -r '(.dependencies//{}),(.devDependencies//{}) | keys[]' package.json
jq -r '(.require//{}),(.["require-dev"]//{}) | keys[]' composer.json
bun pm ls                                 # installed packages
composer show --direct                    # installed packages
```

## References

- [updating-claude-documentation](../updating-claude-documentation/Skill.md) - Style guide and update process
