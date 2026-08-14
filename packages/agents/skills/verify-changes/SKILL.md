---
name: verify-changes
description: Verify the whole changeset the way the User meets it: the suite, the Review gate, the Critical Paths, once every Task has landed. TRIGGER when the last Slice is staged, when an autonomous run reaches functional completion, or on "verify the changes". DO NOT TRIGGER with Tasks still open, where the Orchestrator judges each Task's returned Evidence instead.
---

# Verify Changes

Runs once, over everything. Run it earlier and the Tasks still ahead rewrite what it proved.

## 1. Run the test suite

The suite passes. /prove governs what counts as a pass.

## 2. Run the Review gate

Run /review over the uncommitted changes.

## 3. Walk the Critical Paths

Run /user-testing over the User-facing behavior the changes touch.

IF the changes touch the User Interface:
### Ask /user-testing for browser testing
It traces code and nothing else until its caller asks.

## 4. Route every finding to its Owner

### Send each finding to the Subagent that wrote the code
Critical blocks.
Never: fixing a finding yourself while coordinating.
