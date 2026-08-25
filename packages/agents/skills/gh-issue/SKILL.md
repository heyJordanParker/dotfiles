---
name: gh-issue
description: Create or update GitHub issues. Writes the durable context an Agent cannot rediscover — the problem, the WHY, the systems it crosses, the impact — and leaves the tactical HOW to the Agent that picks it up. Use when the Architect says "create an issue", "file an issue", "gh issue", "update the issue", or wants to turn a Plan into a trackable issue.
---

# GitHub Issue

- An issue is picked up days or weeks after it is written, and the tree changes many times in between.
- The Agent that picks it up reads the tree in seconds. It cannot recover the problem, the intent, or the impact from any tree.
- So the issue carries the WHO, the WHY, and the WHAT. The Agent that executes it owns the HOW.

## 1. Understand the problem, then stop researching

Take the problem from the Architect's own words. Read only far enough to confirm the problem is real and to name the systems it crosses.

### Stop reading code once you can name the systems involved
The systems and their interaction are what the issue carries. Reading further produces detail that goes stale before the issue is picked up.

### Never describe unread code
Describe only code already read in this Task.

## 2. Write the body from the Template

Every slot is filled with what stays true as the tree changes.

### State the problem as a behavior, never as the change to make
The problem is what the system does today and why that is wrong. A body that opens with the change hides the problem, so the executing Agent cannot find a better path.
Example: "Two Agents editing the same session file overwrite each other, so the later write loses the earlier Agent's Task."
Never: "Add a lock to session_state.py".

### Name systems by their durable name, never by a position inside a file
A system's durable name is the module, the command, the Skill, or the Hook event. Line numbers, line ranges, function bodies, and call order are wrong by the time the Agent starts.
Never: `session_state.py:594`, "the loop at the top of the function", or a pasted code block.
Never: `the config file`, `the auth module`, or `the usual place`.

### Keep exact text the Architect fixed
A name, an error message, a command, or a contract the Architect decided is durable. Write it exactly.

### Record every rejected option with its reason
An unrecorded rejection gets rediscovered and re-chosen weeks later. One line each.

### Write each acceptance criterion as an outcome
A criterion states what is true for the User or the system when the work is done, and stays true however the Agent builds it.
Never: a criterion naming a file, a function, or a test name.

### Inline every source the implementing Agent needs
The body never points back to conversation Context, external files, Plans, or Shaping. It carries the needed content itself.
Never: `see above`, `as discussed`, `see the shaping doc`, or `see the Plan file`.

Template:
  ```markdown
  ## Problem
  <what the system does today, who it hurts, what it costs>

  ## Why this matters
  <the intent behind fixing it, in the Architect's words>

  ## Systems involved
  <the systems by durable name, and how they interact today>

  ## Constraints
  <boundaries that hold, options already rejected and why, capabilities that must not regress>

  ## Done when
  <outcome criteria>
  ```

## 3. Audit the body for durability

Ask one question of every sentence: is this still true if the tree changes ten times before the Agent starts?

IF a sentence fails that test:
### Raise it to the durable level, or cut it
A line number becomes the system that owns the behavior. A code block becomes the interaction it showed. A step-by-step HOW becomes the constraint that HOW was serving.

IF a Template slot is empty:
### Fill the slot before creating or updating the issue
Do not run `gh issue create` or `gh issue edit` until every slot is filled.

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
