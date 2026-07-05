---
name: solo
description: Framework for working solo — no Subagents, deep full-file reads, thorough research before acting. TRIGGER when Context can absorb the full picture. For Orchestration, use /subagents or /team.
---

# Solo

Solo means no Subagents and no delegation.
The `enforce-solo-mode` Hook blocks Subagent dispatch.
Read the full Architecture before touching it; fix or propose from root causes at the right layer.

## 1. Confirm solo is the right operating mode

IF the Task has parallel independent work:
### Use /subagents instead
Solo is for work where Context can absorb the full picture.

IF the Task needs persistent multi-Task coordination:
### Use /team instead
Teams preserve Context across turns and Slices.

## 2. Locate the relevant files

### Use the trace Skill to research
Use /trace to locate files and symbols. Do not start with raw Read or grep.

## 3. Read every relevant file fully

### Read whole files
Never offset or limit files under 500 lines.

### Follow every reference
Follow imports, callers, siblings, tests, and configs.

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
Run the relevant tests and report the observed output.
