---
name: present
description: Shape the reply for the Architect - a sentence for a fact, a one-screen packet for a status or finding, a Decision Hierarchy for an Architectural discussion, impact stated before mechanism. TRIGGER on every reply that carries a Decision, a finding, or an Architectural discussion, and on /present. DO NOT TRIGGER for drawing a single view (that is /show-me) or one option set (that is /pcc).
---

# Present

The reply is the product. The Architect reads the last message only, and everything you did exists for him only through it. Thinking and presenting are different acts: a reply written while thinking reads like thinking. So work the answer out first, then write it for the reader. Written for him means what he asked, what he must decide, and what he loses by not knowing.

## 1. Draft in think.md

1. Write the draft to think.md in the session scratchpad: the full answer, every finding, the reasoning. This file is for you.
2. For a final report, the draft starts as a numbered inventory of every Decision made, fix shipped, revert, root cause, open finding, and gate result — from the first message of the session to the last, not just the recent ones.

## 2. Edit the draft into the reply's shape

Pick the shape from the question, then edit the draft into it: impact first, every fact attached to a Decision, everything else cut.

- A fact: one sentence.
- A status or finding: the verdict first, with User impact. Then each open Decision with options, one-line tradeoffs, and your pick. Close with what you need from the Architect. One screen.
- An Architectural discussion: a Decision Hierarchy. The Decision that gates the rest comes first, each Decision nests under the one that gates it, and the close separates what is open for the Architect from what is settled. Three screens; the gating Decision and its children fit the first.
- One option set: /pcc.

### Carry every change's impact, in this order
WHY: the problem it answers, in the Architect's words. User impact: what the User sees, what data is deleted, changed, or untouched, which live pages shift. Where to see it: the page, URL, or command where it is visible. Architecture impact: before and after as two block charts (/show-me owns the drawing), and how the change fits the rest of the system.

### Cut what serves no Decision
Cut process, dead ends, and search paths. Cut every fact that serves no Decision in the reply. Over budget: cut facts, never Decisions.

## 3. Check and send

IF the reply is a final report:
### Check the report against the inventory item by item
A report that drops one item settled earlier in the session is the failure, however clean the rest reads.

Send the edited reply. The draft never reaches the Architect.

IF invoked as /present:
### Re-present the previous reply
Rebuild the previous reply through the same Process and send only the result.
