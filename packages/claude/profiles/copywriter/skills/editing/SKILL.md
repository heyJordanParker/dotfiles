---
name: editing
description: Use when copy already exists and an editor runs its ship-gate checks before the copy clears — line (reads clean), voice (sounds like this product), slop (nothing generic), or strategy (earns the next step). Holds the four disjoint check groups, the sole de-slop checklist, and the additive distinctiveness check. TRIGGER when reviewing, sweeping, tightening, proofing, or de-slopping a draft, or when an editor agent loads it by name. DO NOT TRIGGER for writing new copy from a brief — that's copywriting.
---

# Editing

The gate every piece passes before it ships. The four tests in the profile's `Claude.md` — clear, persuasive, concise, true to voice — name the bar; this skill is how the bar gets applied. The work splits across four editors, each owning one disjoint group of checks. No check lives in two groups, so they can run without stepping on each other.

## The ship gate

- Copy clears only when **every check in your group passes**. One fail holds the piece.
- Each check is **pass/fail on a concrete point** — a phrase that is there or isn't, a claim that's backed or isn't. Not a score, not a vibe.
- You **review by default, and you can write.** Return `{check, pass/fail, the exact line, why it failed}` so the writer revises and you re-check. When making the fix yourself is the faster path, make it and name the check it answered.
- **Distinctiveness is the one additive check** (S5, owned by slop). Every other check fails on something *present* that shouldn't be — a hedge, a passive, an AI tell. Distinctiveness fails on something *absent*: copy can pass every removal check and still be forgettable. It is the counterweight that stops de-slopping from sanding a draft down to inoffensive mush.

## Find your group

Run only your group. The other three are someone else's checks.

- **line-editor** → [Line](#line--reads-clean)
- **voice-analyzer** → [Voice](#voice--sounds-like-this-product)
- **anti-slop-editor** → [Slop](#slop--nothing-generic) — you are the sole owner of de-slop
- **senior-copywriter** → [Strategy](#strategy--earns-the-next-step)

The chief running a small ask inline checks all four. Final tick-list for any group: [references/checklist.md](references/checklist.md).

## Line — reads clean

The sentence as a mechanical object: does it read clean on the first pass? Line owns *how it reads*, never *whether it persuades* (that's Strategy).

- **L1 Clarity** — every sentence understood on one read. Fail: a sentence doing too much, a pronoun with no clear referent, jargon the reader doesn't share. *"The thing that it does, which is what makes it work, is the part teams rely on"* fails — nothing resolves.
- **L2 Active voice** — the verb carries the sentence; subject acts. Fail: passive where active works (*"leads are captured by the form"* → *"the form captures leads"*), a nominalization burying the verb (*"make a decision"* → *"decide"*, *"provide assistance"* → *"help"*), an expletive opener standing in for a real verb (*"there is / there are / there were," "could be heard"* — *"There were a great number of dead leaves lying on the ground"* → *"Dead leaves covered the ground"*), or one passive chained on another (*"It has been proved that he was seen to enter"*).
- **L3 Word economy** — no word the reader doesn't need. Fail on filler intensifiers (very, really, just, actually, simply, literally, truly), dead connectives (*in order to* → *to*, *the fact that* → *that*), and a removable *that*. Read the line without the word; if the meaning survives, the word was tax. At the sentence level, fail a chain of thin sentences carrying one idea where one sentence holds it — fuse them (a ~55-word run of *and*-strung clauses down to a single 26-word sentence).
- **L4 Plain words** — no pompous word where a plain one exists: utilize→use, leverage→use, facilitate→help, implement→set up. Fail on any complex word with a plain twin, and on an empty abstraction noun standing in for the concrete thing — *case, character, nature, system, factor, feature* used as padding (*"acts of a hostile character"* → *"hostile acts"*; *"his training was the great factor in his win"* → *"he won by being better trained"*). Full list and the abstraction nouns: [references/plain-english.md](references/plain-english.md).
- **L5 Rhythm** — read it aloud. Fail if you stumble, if every sentence is the same length, if clause after clause is strung together by *and / but / who / which / while* into one slack line, or if the key word is buried at the tail instead of front-loaded. The pass is varied length — recast a monotonous run into a mix of simple, compound, and periodic sentences — and a strong opening word. Two deliberate devices pass, never fail: a periodic sentence holding the key word to the end for a single climactic line (front-loading stays the default; this is the one admitted exception), and a fragment used for earned emphasis (*"Again and again he called out. No reply."*).
- **L6 Positive form** — say what a thing is, not what it isn't; reserve *not* for real denial or antithesis. Fail on a needless negative with a positive twin (*"he was not very often on time"* → *"he usually came late"*; *"did not pay any attention to"* → *"ignored"*).
- **L7 Related words together** — every modifier sits next to what it modifies. Fail on a misplaced modifier that bends the meaning (*"he only found two mistakes"* → *"he found only two mistakes"*).

## Voice — sounds like this product

Does it read like *this* product and only this product, the same on every surface? Checked against the project's `Voice.md`.

- **V1 Voice match** — matches `Voice.md`'s is/is-not core, lexicon, and register. Fail on anything the is-not list rules out, or a register the voice doesn't use. When the project's `Voice.md` is provisional or absent, fall to the four tests in the profile's `Claude.md` and flag that the check ran against the shared bar, not a confirmed voice.
- **V2 One voice** — consistent end to end. Fail on a shift formal↔casual mid-piece, a swing in person (*you* drifting to *we* drifting to *the company*), or tone whiplash (playful headline, corporate body).
- **V3 Terminology** — uses the terms `Voice.md` says to use, avoids the ones it says to avoid. Fail on a banned term or a competitor's word for the thing when the product has its own.

## Slop — nothing generic

Sole owner of de-slop. Everything an AI or a lazy template would write, gone — and what's left still has a pulse. Full armory: [references/ai-tells.md](references/ai-tells.md).

- **S1 No AI tells** — none of the AI-generated phrasings: *delve, that being said, it's worth noting, at its core, in today's [anything], this begs the question, navigate the landscape*. Fail on any from the list.
- **S2 No em-dash crutch** — em dashes not used as a sentence-variety shortcut. One deliberate em dash is fine; a second on the same screen fails. Most belong as a comma, colon, or full stop.
- **S3 No SaaS slop** — none of revolutionize, supercharge, unlock, elevate, game-changing, next-level, seamless, robust, cutting-edge, world-class. Fail on any unbacked superlative too (*the best way to…* with nothing behind it).
- **S4 No AI structure** — none of the giveaway shapes: *"whether you're X, Y, or Z,"* the additive *"it's not just X, it's Y"* inflation, a sentence opening *"By [verb-ing], you can…"*. The contrarian reframe *"X isn't A, it's B"* — a real claim that corrects the reader — is legitimate, not this tell. Fail on the shape even when the words are clean.
- **S5 Distinctiveness** (additive) — would any line be missed if it were cut? Could a competitor paste this onto their own page unchanged? If yes, it's forgettable — **fail**. This is the only check that fails on absence: a draft can clear S1–S4 and still die here by being merely inoffensive. Clear-and-forgettable loses.

## Strategy — earns the next step

The claim as a persuasion object: does each line move the reader toward the one action? Strategy owns *whether it persuades*, never *how it reads* (that's Line).

- **ST1 Awareness match** — pitched to where the reader actually is. Fail when cold copy skips naming the problem, or warm copy re-pitches a reader who only needs the next step. Sophistication fits the market, not a fresher one.
- **ST2 So what** — every claim answers "why should I care?". Fail on a feature with no bridge to the reader's outcome (*"AI-powered analytics"* with no *"so you decide in half the time"*). The reader's get leads; the mechanism follows.
- **ST3 Specific** — concrete beats vague. Fail on *fast / powerful / better / improve your workflow* where a number, timeframe, or named outcome was available (*"set up in 4 minutes," "cut reporting time in half"*).
- **ST4 Proof** — every promise names what backs it. Fail on an unearned superlative, an unbacked stat, or — hard fail — any invented proof (fake number, testimonial, customer name). A claim with no backing gets flagged to the writer, never fabricated.
- **ST5 One job, clear ask** — one action the piece drives toward, one specific CTA. Fail on a second competing CTA, a buried or hedged ask (*"Learn more," "Get started today!"* over *"Start your first project"*), or an objection left standing next to the CTA with no risk reversal.
- **ST6 Emotion** — the copy makes the reader feel the pain or the want, not just receive information. Fail on flat, purely informational copy where the stakes are real but unfelt.

## References

- [checklist.md](references/checklist.md) — the whole gate, all four groups, terse tick form for a final run-down
- [plain-english.md](references/plain-english.md) — pompous→plain word swaps (Line L4)
- [ai-tells.md](references/ai-tells.md) — AI tells, the em-dash rule, SaaS slop, banned superlatives, AI structural patterns (all of Slop)
