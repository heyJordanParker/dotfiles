---
name: build
description: The build mode's contract — the Agent does every edit and run itself, with no Subagents, deep full-file reads, and research before acting. TRIGGER when the session enters build mode, on the /build command, and when build mode is re-injected after compaction. DO NOT TRIGGER for orchestrate mode, where Subagents do the work (that is /orchestrate).
---

# Build

Build mode means no Subagents and no delegation.
The `block_spawning` Hook refuses a dispatched build Agent's spawns.
Read the full Architecture before touching it; fix or propose from root causes at the right layer.

## 1. Confirm build is the right operating mode

IF the Task has parallel independent work:
### Use /orchestrate instead
Build mode is for work where Context can absorb the full picture.

## 2. Locate the relevant files

### Use the trace Skill to research
Use /trace to locate files and symbols. Do not start with raw Read or grep.

## 3. Read every relevant file fully

### Follow every reference that can affect the requested behavior
Follow the imports, callers, siblings, tests, and configs the behavior runs through. Stop at a stable contract and at an external boundary.

### Read surrounding Architecture
The file with the symptom is rarely the file with the problem.

## 4. Assess whether the issue is real

### Trace the code path before deciding
Confirm behavior against the code path you read. If the issue is not real, report findings and stop.

### Do not hedge about readable code
Never write "likely" or "probably" about code you can read.

## 5. Act at the layer that owns the responsibility

### Assume existing code is intentional
If the reason is unclear, research more before changing or proposing.

### Fix the cause, not the symptom
Change or propose at the layer that owns the responsibility.

## 6. Verify

### Trace the changed code paths
Trace changes through the code paths you read.

### Run tests
Run the tests the change reaches.
