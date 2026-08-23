---
name: pcc
description: Add pros, cons, and confidence to options in a Prompt. TRIGGER when the Architect asks for "/pcc", pros/cons/confidence, alternatives, options, approaches, or a ranked choice. DO NOT TRIGGER for a single implementation call, a convention Decision with repo Precedent, or a proposing-state turn where /propose is canonical.
---

# Pros Cons Confidence

- /pcc compares two or more viable options at one Decision layer.
- /propose is canonical inside a proposing-state turn.

## 1. Confirm /pcc applies

### Use /pcc only for real choices
Use /pcc only when there are two or more viable options. With one option, present the Proposal directly without pros, cons, or confidence.

### Keep one Decision layer
Every option in one /pcc sits at the same layer: Architecture, convention, or implementation. Do not mix layers.

IF the Decision is a convention Decision with repo Precedent:
### Apply the Precedent instead
Find the repo Precedent and use it. Promote to Architecture only when Precedent is missing or needs changing.

IF the Decision is implementation:
### Recommend directly
You own implementation Decisions.

IF the current turn is a proposing-state turn:
### Use /propose instead
/propose carries the pros, cons, and confidence shape on each Decision node.

## 2. Filter the options

### Keep Architecturally distinct options
Each option takes a different approach. Cosmetic variants of the same idea are Fluff.

### Drop options that cannot win
Do not include padding, a "do nothing" or "keep the current approach" option — asking for options already means a change is requested — an option under 35% confidence, or one the User already rejected in this conversation.

### Do not split a picked option into sub-part variants
Once the User picks an option, apply their refinement to it directly.

## 3. Rank confidence

### State confidence as a percentage
No hedging. Never write `might work` or `should be fine`.

### Rank against User, Architecture, and business
Confidence is your percentage that the option is the right call for the User, the Architecture, and the business. It is informed by all research to date, not just this turn. Implementability never raises confidence. Major compromises against those three lower it.

### Keep numbers coherent
Complementary options cannot both be high. Options clustered within roughly 10 points mean you have not differentiated them.

### Treat forced pros, cons, or confidence as a research gap
If scores cluster, pros or cons feel forced, or the ranking is unclear, read more code. Do not adjust numbers to make the Template look finished.

## 4. Write the options

This Template is the one option format. /propose and the proposals rules defer to it.

Template:
  **Option N: [Name]**

  What: 1-2 sentences, concretely, in our code.
  Precedent: the exact file or system this option builds on, named with its full path — or the research proving none exists.

  ```diff
  + how this option solves the stated problem
  - the real cost it adds, the one not seen until it bites
  ```

  Confidence: 82%

### Keep every pro and con one sentence, its marker on every line
One sentence per item. A sentence that wraps repeats its `+` or `-` at the start of each continuation line, so the coloring holds at any width. A cost needing more space names the additional work the option requires, inside that sentence.

### Keep pros and cons inside the option
Pros describe how this option solves the stated problem. Cons describe real costs or risks this option introduces.

Never: cross-option references like `more files than Option Z`, normal implementation cost dressed as a flaw like `touches multiple modules`, or Fluff cons added to balance the Template.

### Do not invent cons
If an option has no real con, say so.

### Remove inevitability framing
A con is a problem the option must attack, not something the User signs off on. If a con stands, fold the solve into the option or name the work the option owes.

Never: `accept`, `accepting`, `live with`, `the price we pay`, `tradeoff we absorb`, `you'll need to accept`.
