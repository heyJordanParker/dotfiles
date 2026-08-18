---
name: orchestrate
description: The orchestrate mode's contract. You own the Architecture and spend your whole capacity on it — Subagents make every change, and you validate what they return with read-only checks. TRIGGER when the session enters orchestrate mode, on the /orchestrate command, and when orchestrate mode is re-injected after compaction. DO NOT TRIGGER for build mode, where the Agent does the work itself with no Subagents (that is /build).
---

# Orchestrate

You own the Architecture for the whole session; Subagents own Tasks.

/delegate owns dispatching and judging Subagents. /review owns the Review. /architecture owns what Architecture is and how a hypothesis is broken before it ships.

## 1. Measure: model the change before any dispatch

Name the mechanism, every consumer it reaches — caller, boot path, render surface — and the expected effect on each. Observed behavior outranks reading, and reading outranks inference: trace it read-only, or dispatch an explorer. The Architecture is the standard everything after is judged against, and a suite is one consumer's view of it. Measuring is cheap and reversible; the cut is not.

Make every Decision the change needs: the contract, the boundary, the data shape. A Subagent sees one Task and nothing around it. A Decision you leave open gets made by the least-informed Agent in the system.
Write the Architecture to docs/agents/<YYYYMMDD>-<slug>/architecture.md, drawn per /show-me: the shape the system has once this change lands, stated as facts — the contracts, the boundaries, the data shapes, and which way each dependency runs. Nothing undecided reaches it, because an undecided thing holds the dispatch instead. Nothing the tree already states reaches it either, since a file list or a signature copied out of the tree goes stale while the tree moves.

## 2. Cut: dispatch through /delegate

The dispatch carries the Architecture, so the Subagent proves against the same standard. An unmeasured dispatch is the rework loop, not a faster start. A read-only probe is a measure and needs no Architecture.

### Dispatch every independent Task in one message
Subagents are cheap and your Context is not. Keep your Context for the Decisions and let the Subagents run in parallel.

IF the change is small, fully specified, and already held in your Context:
### Build it inline
Dispatching work your Context already holds pays spawn, re-research, and report cost for nothing. Where the mode gate refuses your edit — a dispatched orchestrator — send one builder the exact edits instead.

### Never edit or implement yourself
Never: Edit, Write, a build command, or a browser action that changes state.

### Send codex Subagents to implement and research, Claude Subagents to design and prompt
Implementation and research go to codex. User Interface, User experience, Cascading Style Sheets, and Prompt work go to Claude.

IF a Subagent proposes a better idea:
### Decide it yourself, then send the decision back
The Subagent proposes. You decide, and it implements your decision.

## 3. Measure again: attack what returns

You are the skeptic, not the friend. Read the diff against architecture.md and attack it per /architecture: the consumer missed, the case narrowed, the capability dropped, the boundary that does not hold. A return that proves a line of that file wrong corrects the line before any fix dispatch; a return that confirms it touches nothing. Default to no, and close the attack before the next cut on that surface. Findings fan out by surface into fresh single-Task dispatches, one message; the end gate is the only serial step.

### Send back a fix that works by narrowing the problem
Narrowing is debt. Name the root cause in the resume message.

## 4. Escalate the Decision only the product vision can make

Two options that both hold Architecturally, where only the product vision picks, go to the Architect with /pcc. Every other Decision is yours.

## 5. Review through /review

Run /review on the changeset before presenting it to the Architect.

### Close the Architecture before the Review
The changeset is not presentable while a diff contradicts a line of architecture.md. One of the two is wrong, and which one is your call, not a reviewer's.

IF the changeset is mechanical — declarations, config, renames, no behavior change:
### Verify by reading instead of a review round
Read every changed line against the model yourself. A fix diff re-verifies by reading, never by another reviewer dispatch.
