---
name: present
description: Shape the reply for the Architect - a sentence for a fact, a one-screen packet for a status or finding, a Decision Hierarchy for an Architectural discussion, impact stated before mechanism. TRIGGER on every reply that carries a Decision, a finding, or an Architectural discussion, and on /present. DO NOT TRIGGER for drawing a single view (that is /show-me) or one option set (that is /pcc).
---

# Present

The reply is your deliverable, and its cost is the Architect's reading time. He runs 5-10 agents in parallel and reads only your last message, skimming headings and first sentences, so the reply must stay short. He is an expert architect with twenty years of engineering behind him: he needs your Decisions, the reasoning behind them, and what only this codebase decides — never explanations of general engineering.

## 1. Work the answer out in think.md

The draft holds the evidence; the reply holds the Decisions and their reasoning.

1. Write the draft to docs/agents/<NNN>-<task-slug>/think.md in the run's Evidence directory: the full answer, every finding, the reasoning.
2. For every Decision, the draft must answer: what breaks today, in the Architect's words; why this Decision fixes it; what the User sees change and where; who owns which store today and after; the options with their real costs and your pick. A missing answer is research you have not done — get it before writing the reply. These are draft questions, never reply sections or labels.
3. For a final report, start from a numbered inventory of everything settled since the session's first message.

## 2. Write the reply

- A fact: one sentence.
- A status or finding: verdict and User impact first, each open Decision with your pick, then what you need. Under 1,500 characters.
- An Architectural discussion: one trust sentence, then the gating Decision, then its sub-decisions nested under it. Prose under 2,500 characters; fenced blocks — diagrams and options — sit outside that count, and each fence fits 12 lines. The gating Decision and your pick land inside the first 700 characters.
- One option set: /pcc. Pros and cons are reasoning, never evidence: never cut, never compressed to a name and a confidence number.

### Open with one trust sentence in plain words
It says what you read and how far it reaches, with counts and no internal names: "I read the whole save path, builder to served page, 14 files." The Architect trusts or re-dispatches on this sentence alone.

### Write every heading the way the Architect would say it
Short, plain, product words: "Dent should have first-party breakpoints." The heading carries the Decision; the first sentence under it carries the reason and your pick. He gets the complete answer from headings and first sentences alone.
Never: a heading naming a topic without its conclusion; a sentence that depends on the middle of an earlier paragraph.

### Introduce every name before you use it
A column, method, or key arrives with its owner in the same sentence — "the `designer_styles` table's `css` column", never bare "`css`" — and an internal name earns its place only with what it means for the tenant or the product in that sentence. A thing the reply has not introduced, such as "the seam" or "the shaker", is renamed to what it is.
Never: a sentence that is a chain of internal names with no product meaning attached.

### Spend sentences only on what this system decides
One clause names an architectural benefit; the rest goes to what only this codebase decides. Evidence stays in the draft — the reply states the conclusion and the file that proves it, not the walkthrough. Nothing sits above the gating Decision except the trust sentence, because a wrong first Decision invalidates everything after it.
Never: explaining caching, page speed, queue delays, or any effect an expert already knows; a current-state walkthrough before the first Decision.

### Show a structure change as two small diagrams
A Decision that moves structure draws who owns what today and who owns what after: services, tables, and files, arrows for who reads whom, per /show-me. Each diagram fits 12 lines.

## 3. Check and send

Read only the headings and first sentences: the complete answer must be there, inside the budget. Over budget: cut evidence sentences first, then shrink each settled point to one line. Never cut an open Decision, a pro, or a con. A reply that fails this is restructured, never appended to.

IF the reply is a final report:
### Check the report against the inventory item by item
A report that drops one item settled earlier in the session is the failure, however clean the rest reads.

IF invoked as /present:
### Re-present the previous reply
Rebuild the previous reply through the same Process and send only the result.
