---
name: propose
description: |
  Mandatory contract for every proposing-state turn. The classifier names it when the Architect types /propose. Routes every decision to its layer, researches until the Proposal names files, methods, and data changes, orders the Decisions by dependency, shows each one with its code and options, and fixes the findable flaws before sending. TRIGGER on every proposing-state turn — the classifier mandates this. DO NOT TRIGGER for executing turns (that is /execute) or auto turns (mixed intents resolve action first). For the pros/cons/confidence ranking, /pcc is canonical; for the opening maps, /show-me is canonical.
---

# Propose

- A Proposal is a specific solution the Architect can correct: named files, methods, and data changes. He cannot correct a direction.
- The Architect owns Architecture Decisions. Conventions follow repo Precedent. Implementation is yours.
- He corrects the Proposal and you improve it, until he accepts it. Bring the version you cannot improve yourself.

## 1. Route every decision

- Architecture — new or removed APIs, module boundaries, contracts, schema changes, files, packages, a convention replaced: find the options with /discover; the Architect decides.
- Convention — naming, error handling, sync versus async, injection style: apply the repo Precedent.
- Implementation — control flow, data structures, queries, error text: decide it yourself and continue. Stop only when a wrong call would invalidate the work ahead.

## 2. Research until the Proposal is specific

Read the code every claim rests on. Run what can be run here: the command, the API call, the page. Reuse an existing surface before creating one, name every noun the way the project names it, and stay inside every boundary the Claude.md files and the Architect set.

## 3. Order the Decisions

The dominant Decision first, each dependent Decision nested under the Decision that decides whether it exists, settled Decisions final. A settled Decision that blocks the only path is surfaced, not reopened.

## 4. Show the Proposal

Open with the whole-change map per /show-me. Show the code each Decision turns on. Render every good option in full per /pcc, or render the only viable option with no option format. Every question the Architect has not answered reappears until he answers it.

## 5. Improve it before sending

Find the flaws the Architect would: a premise the code contradicts, a capability removed, a requirement relaxed, a simpler shape that meets the requirements, a hedged claim you can check now. Fix and recheck. Send the version you cannot improve.
