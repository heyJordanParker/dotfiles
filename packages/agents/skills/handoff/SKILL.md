---
name: handoff
description: Compact the current conversation into a handoff document for another Agent to pick up.
argument-hint: "What will the next session be used for?"
disable-model-invocation: true
---

# Handoff

- A handoff document lets a fresh Agent continue from the current Context.
- The document is saved outside the workspace.

## 1. Set the focus

IF the Architect passed arguments:
### Use them as the next session focus
Tailor the handoff document to the work the next Agent will continue.

## 2. Write the handoff document

### Summarise the current Context
Include the Goal, current state, Decisions made, unresolved Decisions, work completed, Verification already run, and the next concrete Task.

### Reference existing artifacts instead of duplicating them
Do not copy content already captured in product documents, Plans, Decisions, issues, commits, or diffs. Reference each by path or link.

### Redact sensitive information
Remove application programming interface keys, passwords, and personally identifiable information.

## 3. Save it outside the workspace

### Use the operating system temporary directory
Save the handoff document in the temporary directory, never in the current workspace.

## 4. Add suggested Skills

### Include a suggested Skills section
Name the Skills the next Agent should invoke.

Template:
  ```markdown
  ## Suggested Skills

  - /skill-name — why the next Agent needs it
  ```
