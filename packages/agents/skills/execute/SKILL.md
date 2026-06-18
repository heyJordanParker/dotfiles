---
name: execute
description: |
  Mandatory contract for every executing-state turn. Loads automatically when the session state is executing — the classifier injects "load /execute" on every executing turn. Covers the one boundary that defines an executing turn: implement the work the architect approved, and the moment it needs an architectural change, stop and escalate instead of making the call. TRIGGER on every executing-state turn — the classifier mandates this. DO NOT TRIGGER for proposing turns (that loads /propose) or auto turns.
---

# Execute

You are implementing work the architect already approved. The cto prompt already governs how you read before editing, fix at the root, preserve every capability, hold scope, and prove it ran. This skill adds the one rule none of those enforce, and no hook can: where an executing turn stops.

## Stop at architecture. Escalate, do not decide.

An executing turn implements. It does not make architectural calls. The moment the approved work requires one, stop, put the decision to the architect with /pcc, and wait. Never make the change and keep going.

Architecture is exactly these — the strict definition in the project Claude.md:

- creating, renaming, or moving a file or folder
- creating, renaming, deleting, or changing a public method
- creating, deleting, or changing database schema
- adopting or removing a third-party dependency
- introducing a pattern the codebase has no precedent for

If the approved work lands without touching any of these, implement it fully — that is the whole turn. If it cannot, the decision is the architect's, and the turn's job is to surface it, not resolve it.

### silent-architecture

Making one of the changes above because stopping feels like friction, then reporting it after the fact — or not at all.

**Bad:** Approved to "make the validator reusable," you extract a new `Validator` class in a new file and report it done. You created a file and a public surface — both architectural — with no proposal.

**Good:** You implement the reuse inside the existing surface. If reuse genuinely needs a new file or a new public method, you stop, put that placement to the architect with /pcc, and wait.

**Why:** The approval covered the behavior, not the shape. A file, a public method, a schema column, or a dependency that outlives this turn is expensive for the architect to reverse — it is theirs to decide.

## Self-check before you mutate

- Does this edit create, rename, move, or delete a file, public method, schema, or dependency? → stop, escalate with /pcc.
- Does it introduce a pattern with no precedent in the codebase? → stop, escalate with /pcc.

If both pass, implement the approved work and prove it ran.
