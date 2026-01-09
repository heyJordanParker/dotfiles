---
name: using-git-worktrees
description: Use when starting feature work that needs isolation from current workspace or before executing implementation plans - creates isolated git worktrees with smart directory selection and safety verification
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Using Git Worktrees

Git worktrees create isolated workspaces sharing the same repository, allowing work on multiple branches simultaneously.

**Core principle:** Systematic directory selection + safety verification = reliable isolation.

## Directory Selection Priority

1. **Check existing directories:** `.worktrees/` (preferred) or `worktrees/`
2. **Check Claude.md** for documented preference
3. **Ask user** if neither exists

## Safety Verification

**For project-local directories:** MUST verify .gitignore contains the directory before creating worktree.

**If NOT in .gitignore:** Add it immediately and commit, then proceed.

**Why:** Prevents accidentally committing worktree contents.

## Creation Steps

1. Detect project name: `basename "$(git rev-parse --show-toplevel)"`
2. Create worktree: `git worktree add "$path" -b "$BRANCH_NAME"`
3. Run project setup (auto-detect from package.json, Cargo.toml, etc.)
4. Verify clean baseline (run tests)
5. Report location and status

## Quick Reference

- **`.worktrees/` exists** — use it (verify .gitignore)
- **`worktrees/` exists** — use it (verify .gitignore)
- **Both exist** — use `.worktrees/`
- **Neither exists** — check Claude.md, then ask user
- **Not in .gitignore** — add immediately + commit
- **Tests fail during baseline** — report + ask

## Red Flags

**Never:**
- Create worktree without .gitignore verification (project-local)
- Skip baseline test verification
- Proceed with failing tests without asking

**Always:**
- Follow directory priority
- Auto-detect and run project setup
- Verify clean test baseline

## References

- [commands.md](commands.md) - Shell commands for worktree operations

## Integration

**Pairs with:** finishing-a-development-branch (cleanup after work complete)
