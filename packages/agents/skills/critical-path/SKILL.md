---
name: critical-path
description: Process for changes with a large blast radius — files with many callers or repo-p95 complexity, and actions that are destructive, hard to reverse, or visible outside the repo. TRIGGER before editing a file whose enrichment context shows callers ≥ 20 or complexity at the repo p95, and before any delete, force-push, schema drop, process kill, push, pull-request operation, shared-infrastructure change, or third-party upload.
---

# Critical Path

- Mistakes here lose Users and revenue directly.
- One Process: measure blast radius, get Architect approval for dangerous actions, then verify the dependent capability still holds.

## 1. Measure the blast radius

Use /trace to measure callers and complexity for what you are changing. Read the nearest Claude.md for the boundary it protects.

### Treat high caller count and repo-p95 complexity as Critical Path

A file with at least 20 callers or repo-p95 complexity requires this Skill before editing.

## 2. Confirm with the Architect before dangerous actions

Approval in one Context never extends to another.

IF the action is destructive, hard to reverse, visible outside the repo, or a third-party upload:
### Wait for Architect approval before acting

Destructive means delete, drop, kill, or overwrite. Hard to reverse means force-push, `git reset --hard`, package removal, or continuous integration change. Visible outside the repo means push, pull request, or shared-infrastructure change.

### Never bypass safety checks

Do not use `--no-verify` or `--no-gpg-sign` to get past a guard.

### Root-cause obstacles instead of clearing them destructively

Do not solve an obstacle with a destructive operation.

IF state is unexpected:
### Treat it as another Agent's in-progress work until traced

Unexpected state includes unfamiliar files, branches, and locks.

## 3. Verify the capability held

Exercise what depends on the change and show the observed output.

### Verification is observed behavior

The Skill is not complete until the dependent capability ran and the output is captured.
