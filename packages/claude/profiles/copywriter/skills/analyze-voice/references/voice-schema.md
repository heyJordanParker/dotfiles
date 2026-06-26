# Voice.md schema

The structure of every `Voice.md`. Build each section in order. A section the tier can't fill yet gets a one-line note saying so, never a generic placeholder. Normal case throughout — this is a data file, not a skill.

## 1. Metadata & confidence

A short block at the top recording how much to trust the rest:

- **Tier** — `aspirational` (0 samples), `provisional` (1–4), or `full` (5+).
- **Sources** — what the profile was built from (sample count and what they were, or "intent questionnaire" for aspirational).
- **Generated** — the date.
- **Reproducibility** — the honest ceiling, stated plainly: structured product voice reproduces well; subtle personal cadence does not. A reader deciding how literally to apply the profile reads this first.

## 2. Identity core (is / is-not)

The center of the file. A short list of paired contrasts — what the voice **is** set against what it is **not**. The "is-not" is what makes each line usable: it draws the line the voice won't cross.

```
- Direct, not blunt — says the thing first, but never at the reader's expense.
- Confident, not hyped — claims what's true plainly; never reaches for "revolutionary".
- Warm, not chummy — talks like a person, not like a buddy with slang and emoji.
```

Every "is" needs a concrete "is-not". A lone adjective describes nothing.

## 3. Lexicon & syntax fingerprint

The mechanical patterns that recur across the samples:

- **Register** — person (second person "you"?), tense (present?), active or passive default.
- **Sentence rhythm** — typical length, whether short punchy lines and longer ones alternate, fragment use.
- **Punctuation habits** — em dashes, colons, lists, the writer's stance on exclamation marks and the Oxford comma.
- **Word grade** — plain Anglo-Saxon verbs vs. latinate; concrete nouns vs. abstraction; numbers in figures or words.

## 4. Signature phrases (verbatim)

Exact recurring phrases pulled from the samples, copied word for word — openers, transitions, the way it frames a benefit, sign-offs. This is the highest-fidelity section: these strings are the voice's fingerprint. Never paraphrase; quote.

For an aspirational or thin provisional profile, this section is sparse or empty — say so rather than inventing phrases.

## 5. Tone by situation

How the voice shifts by context. A table, situation in the left column:

| Situation | How the voice shifts |
| --- | --- |
| Landing page headline | boldest, most benefit-forward register |
| Onboarding / empty state | encouraging, low-pressure, one next step |
| Error / failure message | plain, accountable, no blame on the reader |
| Pricing | confident and specific, no hedging or apology |
| Lifecycle email | familiar, one ask, reads like a person not a broadcast |

Trim or extend rows to the surfaces this product actually ships.

## 6. Terms: use / avoid

This writer's specific vocabulary — not the generic banned-word list (that lives in the editing skill):

- **Use** — domain words, product names, framings the writer reaches for and the team should keep consistent.
- **Avoid** — words this writer specifically rejects, on top of the team's defaults. Capture the writer's own forbidden list, drawn from the samples or the questionnaire.

## 7. Anti-patterns

The specific ways copy stops sounding like this voice — failure modes phrased as banned moves, each with what to do instead. These are sharper than the use/avoid list because they name a behavior, not a word:

```
- Stacking three adjectives where one concrete claim would do.
- Opening with a windup ("We're excited to...") instead of the point.
- Hedging a call-to-action ("maybe give it a try") — the voice asks plainly.
```

## 8. Before / after examples

The most teachable section. Two or three pairs: a flat, generic line, then the same line in this voice. The contrast carries more than any rule.

```
Before: Our powerful platform helps teams collaborate more effectively.
After:  Stop losing decisions in chat. Every call your team makes, in one place.
```

## 9. Avatar (optional)

A one-paragraph persona of the voice — if the product's voice has a clear character, sketch who it sounds like (a sharp founder, a calm expert, a friend who's done this before). Skip it when the voice is purely structural; an invented persona is worse than none.
