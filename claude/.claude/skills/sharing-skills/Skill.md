---
name: sharing-skills
description: Use when you've developed a broadly useful skill and want to contribute it upstream via pull request - guides process of branching, committing, pushing, and creating PR to contribute skills back to upstream repository
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Sharing Skills

Contribute skills from your local branch back to the upstream repository.

**Workflow:** Branch → Edit/Create skill → Commit → Push → PR

## When to Share

**Share:** Broadly applicable, well-tested, documented, follows writing-skills guidelines.

**Keep personal:** Project-specific, experimental, contains sensitive info, too narrow.

## Prerequisites

- `gh` CLI installed and authenticated
- Working directory is skill repository clone
- Skill tested using TDD process

## Workflow

### 1. Sync with Upstream

```bash
git checkout main
git pull upstream main
git push origin main
```

### 2. Create Feature Branch

```bash
git checkout -b "add-${skill_name}-skill"
```

### 3. Create or Edit Skill

Work on your skill in `skills/skill-name/Skill.md`

### 4. Commit

```bash
git add skills/your-skill-name/
git commit -m "Add ${skill_name} skill

Brief description. Tested with: [approach]"
```

### 5. Push to Fork

```bash
git push -u origin "add-${skill_name}-skill"
```

### 6. Create PR

```bash
gh pr create \
  --repo upstream-org/upstream-repo \
  --title "Add ${skill_name} skill" \
  --body "## Summary
Brief description.

## Testing
How you tested this skill.

## Context
Why this skill is needed."
```

## After PR is Merged

```bash
git checkout main
git pull upstream main
git push origin main
git branch -d "add-${skill_name}-skill"
git push origin --delete "add-${skill_name}-skill"
```

## Important

**Do NOT batch multiple skills in one PR.** Each skill should have its own branch and PR.
