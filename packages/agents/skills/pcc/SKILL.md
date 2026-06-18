---
description: Add pros/cons/confidence to solutions. Append to any prompt.
---

When presenting alternatives, options, or approaches, use this format. When the ranking sits inside a proposing-state turn, /propose is the canonical contract — it covers the seven named proposal failures (hedging included) and the choice-block shape these options render in.

## Rules

- **Architecturally distinct solutions** — each option takes a fundamentally different approach, not cosmetic variations of the same idea
- **No bad options** — every option must be genuinely viable. Don't pad the list.
- **No rejected options** — if the user already rejected an approach in this conversation, don't resurface it
- **No hedging** — state confidence as a percentage, not "might work" or "should be fine"
- **No sub-part variants** — once the user picks an option, apply their refinement to it directly. Never generate variants of a sub-part of a picked option
- **Two or more viable options required** — /pcc compares options. With one option, present it as the proposal itself without pros/cons/confidence. A single option is not a recommendation — recommendations rank multiple options. Never wrap a single option in the /pcc format
- **Pros and cons stay inside the option** — pros describe how this option solves the stated problem; cons describe real costs or risks this option introduces. Forbidden: cross-option references ("more files than Option Z", "more complex than the alternative"), treating normal implementation cost as inherent badness ("8-file edit", "touches multiple modules"), filler cons added to balance the format. If an option has no real cons, say so — don't invent one
- **No "accept" framing** — never tell the user to accept, absorb, or live with a con. Banned words and phrases in any pros/cons/recommendation: "accept", "accepting", "live with", "the price we pay", "tradeoff we absorb", "you'll need to accept". A con is a problem the option must attack, not a compromise the user swallows. If a con stands, either fold the solve into the option or surface it as outstanding work the option owes — never as a thing the user signs off on. With AI doing the typing, solving cons is cheap; framing them as inevitable is the failure mode this rule blocks
- **Confidence ranks rightness against our first principles** — the percentage is how confident you are this is the right call for the user, the architecture, and the business (defined in the cto prompt's Goal section). Start the judgement there, not from the option's mechanics. It is informed by all research to date, not just this turn's. Implementability never raises it; major compromises against those three drag it down. Across options of one decision the numbers cohere — complements cannot both be high. Options clustered within ~10% of each other (88/90/92) mean you haven't differentiated them
- **Inconsistent confidence or pros/cons signal a research gap** — if scores cluster, if pros/cons feel forced, if you can't tell why one option scores higher than another, that's the agent flagging its own lack of codebase research. Fix it by reading more code, not by adjusting numbers or reshuffling bullets. Never ship a /pcc with patched-over scores
- **Single layer per invocation** — every option in one /pcc sits at the same decision layer (architecture / convention / implementation). Mixing is banned.
- **Convention decisions skip /pcc** — find the repo precedent and apply it; promote to architecture only when precedent is missing or needs changing.
- **Implementation decisions skip /pcc** — direct recommendation, agent owns.

## Format per option

**Option N: [Name]** (X% confident)
- What: 1-2 sentences
- Pros: bullets
- Cons: bullets
