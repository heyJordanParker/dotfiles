---
name: pragmatic-engineering
description: Keep engineering boring and complete. TRIGGER when the Architect says KISS, pragmatic, don't overengineer, ship it, keep it simple, or asks for a pragmatic read on a Proposal, Plan, code review, or Architectural Decision. DO NOT TRIGGER when the Architect asks for maximum rigor over shipping pressure.
---

# Pragmatic Engineering

- One Process: ship the smallest complete change that preserves the User's capability and the Architecture.
- Most time savings come from deletion, not acceleration.

## 1. Question the requested work

Prove the requested work serves the User before preserving it. Question Agent-generated work especially hard.

### Read before correcting the Architect

Correct the Architect only after reading the code. Never say "You're absolutely right!" before Verification.

IF you see assumed intent:
### Stop and research before proceeding

Do not continue from a guess.

## 2. Delete

Remove dead paths, duplicated choices, unused indirection, and code that does not serve the Goal. If deletion breaks a capability, restore only the capability.

### Delete more than you create

A pragmatic change removes work before it adds work.

## 3. Simplify

Do not optimize code that should not exist. Keep files small, dependencies one-way, and each module cheap to rewrite.

### Avoid speculative abstraction

Abstract after three concrete uses, never before.

IF you see abstractions "for later" or two-way dependencies:
### Stop and simplify before proceeding

The simplest complete path is the one with the fewest moving parts that still preserves the User capability.

## 4. Use existing code and libraries

Reuse repo Precedent and well-maintained third-party code before writing local machinery. Read the library behavior before building on it.

IF you see duplicated third-party functionality or building before understanding library behavior:
### Stop and research the library first

Never rebuild behavior a library already owns.

## 5. Speed up

Improve performance only after deletion, simplification, and reuse. Developer experience beats runtime performance until measured User harm proves otherwise.

IF you see optimization before deletion:
### Delete first

Do not accelerate a path that should not exist.

## 6. Automate last

Automating unnecessary work preserves the wrong path. Automate only the Process that survived the first five steps.

IF you see automation before simplification:
### Simplify before automating

Automate the surviving Process, not the first Process you found.

## 7. Report failures immediately

State the blocker, the paths attempted, and the ranked options.

IF you see hidden errors or claimed completion without Verification:
### Stop and report the failure

Never work around a failure silently.
