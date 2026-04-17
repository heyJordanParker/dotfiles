---
description: Add pros/cons/confidence to solutions. Append to any prompt.
---

When presenting alternatives, options, or approaches, use this format.

## Rules

- **Architecturally distinct solutions** — each option takes a fundamentally different approach, not cosmetic variations of the same idea
- **No obviously bad options** — every option must be genuinely viable. Don't pad the list.
- **No rejected options** — if the user already rejected an approach in this conversation, don't resurface it
- **No hedging** — state confidence as a percentage, not "might work" or "should be fine"
- **No sub-part variants** — once the user picks an option, apply their refinement to it directly. Never generate variants of a sub-part of a picked option

## Format per option

**Option N: [Name]** (X% confident)
- What: 1-2 sentences
- Pros: bullets
- Cons: bullets
