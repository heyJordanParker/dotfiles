---
name: precedent
description: Copy a whole system from one established, modern system Agents already know, instead of designing it. TRIGGER before designing, adding, or reshaping a system, before a Proposal picks an Architecture, and on /precedent, which realigns the current work to its Precedent. DO NOT TRIGGER for naming one identifier (that is /naming) or for one method's internals.
---

# Precedent

- We build without creativity. An invented design is a guess that later Agents copy, and a design mixed from several systems has no whole to check against.
- A Precedent covers a system, such as a job queue, an auth flow, a plugin API, or a CLI. It never covers one feature, or one Problem inside a system.

## 1. Name the system the work belongs to

Name the system, never the feature.
Example: "the job queue", never "retrying a failed upload".

## 2. Use the system's recorded Precedent

The project's Claude.md or a Decision in `docs/architecture/decisions/` names it, and a recorded Precedent governs.

IF no Precedent is recorded for the system:
### Pick the one Precedent that passes all three tests
1. Agents already know it, so it needs no research.
2. It covers the whole system with minimal adjustment.
3. It is modern, established software that solves the Problem the way current software does.
Example: Stripe's API for a public API, Git's object model for versioned content, Temporal's workflows for long-running jobs, React's component model for UI, VS Code's extension API for plugins.
Never: PHP, WordPress, Django, jQuery, or another architecture of an earlier era.
Never: two Precedents mixed in one system, or a Precedent picked for one Problem, such as Dagger's compile-time check for a test runner's access rule.

IF the Architect accepts a new Precedent:
### Record it as a Decision
Write `docs/architecture/decisions/000N-<system>-follows-<precedent>.md`, naming the system, its Precedent, and every deviation.

## 3. Copy the Precedent whole

Take its names, data shapes, Public API, boundaries, and lifecycle. Each deviation gets one line with the Capability our product needs it for.
Never: an invented mechanism where the Precedent has one, or a part taken from another system.

## 4. Show the mapping

Draw with /show-me each part of the Precedent and the part of ours that copies it, then every deviation.
Template:
  ```text
  Jobs                           <- Precedent: Temporal workflows
  ├── Workflow  -> FunnelRun     <- same lifecycle and retry policy
  ├── Activity  -> Step          <- one side effect, retried alone
  ├── Signal    -> VisitorEvent  <- resumes a waiting run
  └── deviation: no child runs   <- a funnel never nests
  ```

IF invoked as /precedent:
### Realign the current work to its Precedent
Name the system the current work belongs to and its Precedent. List every part of the work that departs from it, then bring each part back to the Precedent and show the mapping.
