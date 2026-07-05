---
name: gh-issue
description: Create or update GitHub issues. Enforces self-contained issues that an Agent can implement without conversation Context. Use when the Architect says "create an issue", "file an issue", "gh issue", "update the issue", or wants to turn a Plan into a trackable issue.
---

# GitHub Issue

- GitHub issues are fully self-contained.
- The implementing Agent has no conversation Context.
- Every missing detail becomes a blocker or wrong assumption.

## 1. Read before writing

Read the affected code, check the nearest Claude.md for Rules the implementing Agent must follow, and gather repo-relative file paths.

### Never describe unread code
Describe only code already read in this Task.

### Use exact repo-relative paths
Name the file path the Agent can open.
Never: `the config file`, `the auth module`, or `the usual place`.

## 2. Write the issue body

Inline the conversation's Plan or content. Include WHY, repo-relative file paths, concrete implementation detail, and acceptance criteria.

### Inline every source the implementing Agent needs
The body never points back to conversation Context, external files, Plans, or Shaping. It carries the needed content itself.
Never: `see above`, `as discussed`, `see the shaping doc`, or `see the Plan file`.

## 3. Check self-containment

Ask whether an Agent can implement the issue without asking a single question.

IF the body is missing WHY, repo-relative paths, inline content, or acceptance criteria:
### Fix the body before creating or updating the issue
Do not run `gh issue create` or `gh issue edit` until the body is self-contained.

## 4. Create the issue

Run `gh issue create`, always pass `--assignee @me`, pass `--repo <owner/repo>` or `--label` when specified, then report the issue URL.

Template:
  ```bash
  gh issue create --assignee @me --title "<title>" --body "$(cat <<'EOF'
  <body>
  EOF
  )"
  ```

## 5. Update the issue

Use `gh issue edit` when the body, title, labels, or state changes. Every edit gets a `gh issue comment` stating WHAT changed and WHY. Body rewrites go through step 3 before editing.

### Add a change comment for every edit
The comment makes the changed issue self-contained for the next Agent.

Template:
  ```bash
  gh issue edit <number> --body "$(cat <<'EOF'
  <updated body>
  EOF
  )"

  gh issue comment <number> --body "$(cat <<'EOF'
  ## Changes

  - <WHAT changed>

  WHY: <WHY these changes were necessary>
  EOF
  )"

  gh issue close <number> --comment "Resolved: <reason>"
  ```
