---
name: build
description: The build mode's contract — the Agent does every edit and run itself, with no Subagents, deep full-file reads, and research before acting. TRIGGER when the session enters build mode, on the /build command, and when build mode is re-injected after compaction. DO NOT TRIGGER for orchestrate mode, where Subagents do the work (that is /orchestrate).
---

# Build

Build mode means no Subagents and no delegation.

## 1. Confirm build is the right operating mode

IF the Task has parallel independent work:
### Use /orchestrate instead
Build mode is for work where Context can absorb the full picture.

## 2. Read the whole call chain before deciding

### Stop at a stable contract and at an external boundary
Follow the imports, callers, siblings, tests, and configs the behavior runs through. The file with the symptom is rarely the file with the problem.

IF the code path does not produce the reported behavior:
### Report what it does instead, and stop
There is nothing to fix.

## 3. Act at the layer that owns the responsibility

### Assume existing code is intentional
Research the reason before you change it or propose against it. Then fix the cause at the layer that owns it, never the symptom at the layer that shows it.

## 4. Prove the change with /prove

Nobody holds the whole changeset for you here, so /prove's end gate is yours: you run the suite, and you attribute a red gate you did not expect.
