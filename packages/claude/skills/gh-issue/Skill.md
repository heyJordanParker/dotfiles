---
name: gh-issue
description: Use when creating or updating GitHub issues. Enforces self-contained issues that an AI agent can implement without conversation history. Use when user says "create an issue", "file an issue", "gh issue", "update the issue", or wants to turn a plan into a trackable issue.
---

# GitHub Issue

Create and update GitHub issues that are fully self-contained. The implementing agent has zero conversation context — every detail must be in the issue body.

## Why

Issues created from conversation silently depend on that conversation. The implementing agent has none of it. Every vague reference becomes a blocker or wrong assumption.

## Creating Issues

### Before

1. **Read affected code** — never describe code you haven't read
2. **Check nearest Claude.md** — capture requirements and boundaries the implementer must follow
3. **Gather file paths** — repo-relative, never vague ("the config file")

### During

The plan/content generated in conversation goes into the issue body as-is. No rigid template — but enforce these rules:

- **WHY is stated** — business motivation, not just technical description
- **All file paths are repo-relative** — never "the auth module"
- **No conversation references** — never "see above", "as discussed", "the usual way"
- **No external references** — never "see the shaping doc" or "see the plan file". Inline the content, don't point to it
- **Concrete implementation details** — an agent with zero context can start working immediately
- **Acceptance criteria included** — how to verify the work is done

### After — Self-Containment Check

Before running `gh issue create`, verify every item:

- [ ] Could an agent implement this without asking a single question?
- [ ] WHY is stated in the body?
- [ ] All file paths repo-relative?
- [ ] No references to conversation, external files, plans, or shaping docs?
- [ ] Acceptance criteria present?

If any check fails, fix the issue body before creating.

### Create

```bash
gh issue create --assignee @me --title "<title>" --body "$(cat <<'EOF'
<body>
EOF
)"
```

Pass `--repo <owner/repo>` if specified. Pass `--label` if specified. Report the issue URL when done.

## Updating Issues

### When

Use `gh issue edit` when the issue body, title, labels, or state need to change.

### Process

1. **Edit the issue** — `gh issue edit <number>` with updated fields
2. **Add a comment explaining the changes** — every edit gets a comment via `gh issue comment <number>`:
   - What changed
   - Why those changes were necessary
3. **Same self-containment check** applies if rewriting the body

### Commands

```bash
# Edit body
gh issue edit <number> --body "$(cat <<'EOF'
<updated body>
EOF
)"

# Add change comment
gh issue comment <number> --body "$(cat <<'EOF'
## Changes

- <what changed>

**Why:** <why these changes were necessary>
EOF
)"

# Close
gh issue close <number> --comment "Resolved: <reason>"
```

## Rules

- Always `--assignee @me` on creation
- Never create an issue that references conversation context
- Never reference external files — inline the content
- Never use vague file references — always exact repo-relative paths
- Never skip the self-containment check
- Always add a change comment when updating an issue body
