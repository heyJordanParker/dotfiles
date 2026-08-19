---
name: execute
description: |
  Mandatory contract for every executing-state turn. The classifier names it when the Architect types /execute, and every 30 turns after that while the session is executing. Implements the work the Architect approved; when that work needs an Architectural change, stop and escalate instead of deciding. TRIGGER on every executing-state turn — the classifier mandates this. DO NOT TRIGGER for proposing turns (that is /propose) or auto turns.
reload-every: 30 turns
---

# Execute

- The Architect already approved the work.
- The `cto` Prompt governs reading before editing, fixing at the root, preserving every capability, holding scope, and proving it ran.
- This Skill corrects where Execution stops.

## 1. Check the approved work against Architecture before editing

Architecture is the files, public APIs, and database, plus third-party dependencies and patterns with no Precedent.

IF the approved work needs an Architectural change:
### Stop before mutating and put the Decision to the Architect
Use /pcc, wait for the Architect, and never make the change while continuing Execution.

An Architectural change is any edit that would:

- create, rename, move, or delete a file or folder
- create, rename, delete, or change a public method
- create, delete, or change database schema
- adopt or remove a third-party dependency
- introduce a pattern the codebase has no Precedent for

Example: implement the reuse inside the existing surface. If reuse genuinely needs a new file or a new public method, stop and put that placement to the Architect with /pcc.
Never: extract a new `Validator` class in a new file after approval only said to "make the validator reusable"; that creates a file and public surface without a Proposal.

Approval covers the behavior, not the shape. A file, public method, schema column, or dependency that outlives the turn is expensive for the Architect to reverse.

## 2. Implement fully when Architecture stays unchanged

Build the approved work completely and prove it ran.

IF the session is in orchestrate mode:
### Dispatch the implementation per /delegate
Judge the returned Evidence instead of editing yourself.
