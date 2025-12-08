---
name: finishing-a-development-branch
description: Use when implementation is complete, all tests pass, and you need to decide how to integrate the work - guides completion of development work by presenting structured options for merge, PR, or cleanup
---
> Originally from superpowers plugin. Copied to personal skills for stability.

# Finishing a Development Branch

Guide completion of development work by presenting clear options.

**Core principle:** Verify tests → Present options → Execute choice → Clean up.

## The Process

### Step 1: Verify Tests

Run project's test suite. **If tests fail:** Stop. Don't proceed until tests pass.

### Step 2: Determine Base Branch

```bash
git merge-base HEAD main 2>/dev/null || git merge-base HEAD master 2>/dev/null
```

### Step 3: Present Options

Present exactly these 4 options:

```
Implementation complete. What would you like to do?

1. Merge back to <base-branch> locally
2. Push and create a Pull Request
3. Keep the branch as-is (I'll handle it later)
4. Discard this work

Which option?
```

### Step 4: Execute Choice

| Option | Actions |
|--------|---------|
| **1. Merge** | Checkout base → pull → merge → verify tests → delete branch → cleanup worktree |
| **2. PR** | Push branch → gh pr create → keep worktree |
| **3. Keep** | Report "Keeping branch" → don't cleanup |
| **4. Discard** | Confirm with typed "discard" → checkout base → delete branch (force) → cleanup worktree |

### Step 5: Cleanup Worktree

**For Options 1, 2, 4:** `git worktree remove <path>`
**For Option 3:** Keep worktree

## Quick Reference

| Option | Merge | Push | Keep Worktree | Cleanup Branch |
|--------|-------|------|---------------|----------------|
| 1. Merge | ✓ | - | - | ✓ |
| 2. PR | - | ✓ | ✓ | - |
| 3. Keep | - | - | ✓ | - |
| 4. Discard | - | - | - | ✓ (force) |

## Red Flags

**Never:**
- Proceed with failing tests
- Merge without verifying tests on result
- Delete work without typed confirmation
- Force-push without explicit request
