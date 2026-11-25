---
name: propose
description: |
  Required on every proposing-state turn. The classifier loads it when the Architect types /propose. It sorts each choice into Architecture, Convention, or implementation, researches until the Proposal names files, methods, and data changes, orders the Problems by dependency, shows each with the Solution to ship and its code, and fixes every flaw it can find before sending. TRIGGER on every proposing-state turn, because the classifier requires it. DO NOT TRIGGER for executing turns or auto turns (mixed intents resolve action first). /show-me draws the opening map.
---

# Propose

- A Proposal is a specific Solution the Architect can correct: named files, methods, and data changes. He cannot correct a direction.
- The Architect owns Architecture Decisions. Conventions follow the repo's existing patterns. Implementation is yours.
- He corrects the Proposal and you improve it, until he accepts it. Bring the version you cannot improve yourself.

## 1. Route every decision

- Architecture: new or removed APIs, module boundaries, contracts, schema changes, files, packages, the names these carry, a convention replaced: copy the system's Precedent with /precedent, and find options with /discover only for what the Precedent leaves open. The Architect decides.
- Convention: naming style, error handling, sync versus async, injection style: apply the repo's existing pattern.
- Implementation: control flow, data structures, queries, error text. Decide it yourself and continue. Stop only when a wrong call would invalidate the work ahead.

## 2. Research until the Proposal is specific

Read the code every claim rests on. Use /understand to research the systems behind the change. Run what can be run here: the command, the API call, the page. Reuse an existing surface before creating one, name every noun the way the project names it, and stay inside every boundary the Claude.md files and the Architect set.

## 3. Order the Problems

Put the biggest Problem first. Nest each smaller Problem under the Problem whose Solution decides whether it exists. Put the Problems the Architect already accepted last. When an accepted Solution blocks the only path, say so, and do not reopen it.

## 4. Show the Proposal

Open with a /show-me map of the whole change. Show each Problem with the Solution you would ship and the code it changes. Repeat every question the Architect has not answered until he answers it.

## 5. Improve it before sending

Find the flaws the Architect would: a premise the code contradicts, a Capability removed, a requirement relaxed, a simpler shape that meets the requirements, a hedged claim you can check now. Fix and recheck. Send the version you cannot improve.
