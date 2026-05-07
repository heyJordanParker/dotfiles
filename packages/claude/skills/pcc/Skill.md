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
- **Two or more viable options required** — /pcc compares options. With one option, present it as the proposal itself without pros/cons/confidence. A single option is not a recommendation — recommendations rank multiple options. Never wrap a single option in the /pcc format
- **Pros and cons stay inside the option** — pros describe how this option solves the stated problem; cons describe real costs or risks this option introduces. Forbidden: cross-option references ("more files than Option Z", "more complex than the alternative"), treating normal implementation cost as inherent badness ("8-file edit", "touches multiple modules"), filler cons added to balance the format. If an option has no real cons, say so — don't invent one
- **Confidence ranks rightness, not implementability** — the percentage reflects how confident you are that THIS option is the right call for the stated problem, after accounting for compromises. Major architectural compromises drag the score down. Options clustered within ~10% of each other (88/90/92) mean you haven't differentiated them
- **Inconsistent confidence or pros/cons signal a research gap** — if scores cluster, if pros/cons feel forced, if you can't tell why one option scores higher than another, that's the agent flagging its own lack of codebase research. Fix it by reading more code, not by adjusting numbers or reshuffling bullets. Never ship a /pcc with patched-over scores

## Format per option

**Option N: [Name]** (X% confident)
- What: 1-2 sentences
- Pros: bullets
- Cons: bullets
