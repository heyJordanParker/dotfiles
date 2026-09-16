---
name: present
description: Present information to the Architect so he can act on it. TRIGGER on every reply that carries a Decision, a finding, or an Architectural discussion, and on /present. DO NOT TRIGGER for drawing one diagram; that is /show-me.
---

# Present

The reply is your deliverable, and its cost is the Architect's reading time. He runs 5-10 agents in parallel and reads only your last message, skimming headings and first sentences, so the reply must stay short. He is an expert architect with twenty years of engineering behind him: he needs your Decisions, the reasoning behind them, and what only this codebase decides — never explanations of general engineering.

## 1. Work the answer out in think.md

The draft holds the evidence; the reply holds the Decisions and their reasoning.

1. Write the draft to docs/agents/<NNN>-<task-slug>/think.md in the run's Evidence directory: the full answer, every finding, the reasoning.
2. For every Decision, the draft must answer: what breaks today, in the Architect's words; why this Decision fixes it; what the User sees change and where; who owns which store today and after; the options with their real costs and your pick. A missing answer is research you have not done — get it before writing the reply. These are draft questions, never reply sections or labels.
3. For a final report, start from a numbered inventory of everything settled since the session's first message.

## 2. Structure the reply as Goal, Problem, Solution

The Architect reads the reply to check you are solving the right Problems and that your Solutions match his vision.

Template:
  ## Goal
  The big idea, alone in this paragraph.

  Then the context he needs about it, one idea per paragraph.

  ### Problem
  The big idea, alone in this paragraph.

  Then its context.

  ### Solution
  The change itself, drawn with /show-me.

  #### Problem      <- a Problem inside a Solution repeats the structure, to any depth
  #### Solution

### Give each level a distinct style
The Architect identifies a level by its style while he skims.

### Rank by importance, never by order of discovery
Two items sit at the same level only when they are equally important.

### Say a thing once in the reply
Repeat what an earlier reply said whenever the Architect needs it now.
Never: "as in the tree above", "step 3 covers this", "see the Solution below".

### Say the problem in the heading
A Problem heading says what the User cannot do. A Solution heading says what changes.
Never: "Decision 1: what a step's rule is".
Never: "Problem: the Goal 2 bar says every server-side option assembly deletes, and one survives it".
Never: "What the report got wrong".
Example: "Problem 2: the cart has two ways to add an offer"; "Solution: the step states how it sells".

### Introduce every name before you use it
A column, method, or key arrives with its owner in the same sentence — "the `designer_styles` table's `css` column", never bare "`css`" — and an internal name earns its place only with what it means for the tenant or the product in that sentence. A thing the reply has not introduced, such as "the seam" or "the shaker", is renamed to what it is.
Never: a sentence that is a chain of internal names with no product meaning attached.

### Say what a name is the first time you use it
A class, method, table, or idea the Architect has not met gets one plain sentence saying what it does for the product. A method the change creates is marked new.
Never: "advance takes the record the Visitor's click created" about a method that does not exist yet.
Never: explaining what caching does, why a queue adds delay, or any general engineering.

### Show a structure change as two small diagrams
A Decision that moves structure draws who owns what today and who owns what after: services, tables, and files, arrows for who reads whom. Use /show-me. Each diagram fits 12 lines.

## 3. Check and send

Read only the headings and first sentences: the complete answer must be there. Restructure a reply that fails this. Never append to it.

IF the reply is a final report:
### Check the report against the inventory item by item
A report that drops one item settled earlier in the session is the failure, however clean the rest reads.

IF invoked as /present:
### Re-present the previous reply
Rebuild the previous reply through the same Process and send only the result.
