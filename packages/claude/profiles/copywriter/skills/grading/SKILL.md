---
name: grading
description: The profile-wide grading standard — nothing is true/false; every claim, problem, buyer, offer, and strategy carries an EVIDENCE rating from 1 to 100 COMPUTED from countable evidence by the published rubric, arithmetic shown, with measurement contracts for market claims. Every rating rides to the owner's pick with its reasoning; nothing is auto-killed by a threshold. TRIGGER whenever any skill or agent states, compares, or hands the chief a claim, problem, buyer, offer, or strategy. DO NOT TRIGGER to run a specific check skill or to gather the evidence itself (the research skills).
---

# Grading

Nothing in this system is true or false. Everything is true to SOME extent, and we are confident about everything to SOME extent. Two agents saying "X is true" and "Y is true" leave the chief picking by disposition; "X, 65 on these countables" versus "Y, 25 on these countables" gives the chief numbers to work with. Weak opinion is good.

### Rate from 1 to 100, never a boolean status
Every judgment is a rating from 1 to 100 with its reasoning, never a PROVEN/UNPROVEN, REAL/FAKE, or pass/fail status. A low rating is a low rating, not a kill: the rating rides to the owner's pick alongside every other, and the owner picks with it visible. Nothing is auto-removed because it fell under a threshold.

## 1. Compute every evidence rating

An EVIDENCE rating is COMPUTED from countables, never assigned by feel. The three inputs:

- **Evidence elements present** — which of the seven evidence elements (the researcher's evidence bar, `agents/researcher.md`) are present, each with its citation.
- **Venue count** — the number of distinct venues the problem is spoken in, from the occurrence model below.
- **Quote form** — supporting quotes fetched verbatim from their URLs, versus paraphrase, versus interpretation with no quote.

### Score by the lookup table

| Input | Points |
|---|---|
| Each of the seven evidence elements present, cited | +10 each (0–70) |
| Venue count: 1 venue | +0 |
| Venue count: 2 venues | +5 |
| Venue count: 3+ venues | +10 |
| Every supporting quote fetched verbatim from its URL | +15 |
| Any paraphrase among the supporting quotes | +5 |
| No quotes — interpretation only | +0 |

Evidence rating = elements + venue count + quote form. Maximum 95.

### The evidence rating is not a probability the problem matters
The evidence rating measures completeness and verification — how much cited, fetched evidence exists — and nothing else. Prevalence, importance, and demand are separate qualitative fields, each defaulting to "we do not know" until a measurement contract (section 2) is met. No artifact may present the evidence rating as the probability the problem is real, recurs, or creates demand. A comparison of two numbers states which number it compares.

### Count occurrence by speakers, incidents, and venues
A problem carries three separate counts, never collapsed into one:

- **Speakers with complete chains** — distinct problem-holders whose own chain carries elements 1–4. A reply author reporting their OWN prior experience is an independent speaker, not part of the thread's incident.
- **Incidents** — distinct events. Several affected holders of one shared incident are several speakers but one incident.
- **Venues** — distinct threads or communities the problem is spoken in.

Element 5 needs 2+ speakers with complete chains from 2+ incidents; the venue-count bonus rewards 2+ venues. Counting the three separately is what stops one multiplicity from scoring twice.

### Count elements by the chain rules
This is the one home for the chain rules; the record templates and researcher contracts reference it, never restate it. Elements 1–4 and 6 are per-speaker-chain: each comes from THAT one speaker, and 1–4 count only when a single problem-holder's chain carries all four. Elements 5 and 7 are problem-level, counted ACROSS chains. Element 5 (independent occurrence) scores its +10 when the evidence carries 2+ speakers with complete chains from 2+ incidents (the occurrence model above): a reply author reporting their own prior experience is an independent speaker; replies describing one shared incident are one incident. Element 7 (spend or behavior change) is scored by its evidence classes below. A fragment stitched across speakers never counts toward a per-speaker element.

### Score element 7 by evidence class
Element 7 is evidence the problem-holder WOULD change behavior or spend money to solve it, scored by the strength of what was observed, each class citing its chain:

- **Completed targeted spend** — money actually paid to solve this problem's failure stage — full weight.
- **Completed targeted behavior change** — a change made to solve that failure, observed after the attempt — full weight.
- **Corroborated stated willingness** — stated intent backed by action (booked calls, an active vendor search) — half weight.
- **Bare stated willingness** — intent with no action behind it — quarter weight, and never alone sufficient to satisfy the element.

Target and temporal conditions bind every class: the spend or change must TARGET this problem's failure stage — buying more of what already failed (more ad spend when the failure is downstream of traffic) is not spend-to-solve — and behavior change counts only when observed AFTER the attempt, never a pre-existing action or a stated future intent.

Element 7 scores its +10 at full confidence when 2+ independent problem-holders each show a full-weight class. One full-weight holder plus one corroborated-willingness holder scores it at reduced confidence. Bare stated willingness alone never earns the points.

### Score an element zero only after continuations are read
An element scores zero only after the chain's thread continuations are fetched and its evidence is genuinely absent. When the continuations are unfetched, the element is unscored — it earns no points and is noted as unread, never counted as absent. Unfetched material earns no element points. Counting an element absent on an unread continuation is a guess, not a count.

### Honesty caps
Applied after the arithmetic, whichever bites lowest, and named beside the rating:

- A problem merged from multiple speakers caps at **85** unless every one of the seven elements is evidenced by 2+ independent speakers.
- Any PARAPHRASE among elements 1–4 caps the claim at **60**.

### Score a BUYER GROUP from existence evidence
A buyer group's evidence rating is COMPUTED from the existence evidence its research records carry — never reused problem arithmetic:

| Input | Points |
|---|---|
| Self-name observed verbatim at its URL (fetched) | +40 |
| Venues where the group self-names: 1 | +10 |
| Venues: 2 | +20 |
| Venues: 3+ | +30 |
| Distinct members observed speaking as the group: 2–4 | +15 |
| Distinct members: 5+ | +30 |

Maximum 100. A group whose self-name was never observed verbatim scores venue and member points only and caps at **40** — a coined segment label has no self-name to observe.

### Score an OFFER SHAPE from verified purchases
An offer shape's evidence rating is COMPUTED from independent VERIFIED PURCHASES of that exact shape by a deterministic ladder. A purchase counts only when the buyer's own words name what they paid for, fetched verbatim at the URL, and what they bought IS the shape the strategy offers — a purchase of a different shape, more ad spend, or a self-made change counts nothing here.

| Independent verified purchases of this shape | Base | Note |
|---|---|---|
| 0 | 20 | unproven demand |
| 1 | 40 | unproven demand |
| 2+ | 70 | evidenced demand |

Two defined bonuses add to the base, capped at 90: **recency** +10 when a verified purchase is dated within the last 12 months; **venue count** +10 when the verified purchases span 2+ venues. Asking for free advice is NOT purchase evidence: a shape supported only by free-advice asks rates **20** with an unproven-demand note, whatever else was counted.

### Score a POSITIONING by its weakest claim
A positioning line makes one or more claims; its evidence rating = the MINIMUM of the evidence ratings of the claims it makes. Each claim should resolve to a graded competitor record (or matched buyer-quote evidence); a claim with no citation carries the minimum evidence rating with its reason stated (no citation found), and that low rating rides to the owner's pick like any other. Nothing is auto-removed for lacking a citation — the missing citation is a reason on the rating, logged as an owner question where the field needs one, never a bar that stops the strategy from reaching a pick.

### Grade a strategy by its weakest part
A strategy's evidence rating = the MINIMUM of its parts' evidence ratings — problem, buyer group, offer shape, positioning — stated with every part's rating shown: `problem 65, group 40, shape 40, positioning 70 → strategy 40`. A strategy rating that is not the minimum of shown parts is invented and does not count as graded.

### Show the arithmetic wherever the rating appears
Every stated rating carries its arithmetic: `5/7 elements (50) + 3 venues (+10) + paraphrase (+5) = 65`. A bare rating with no arithmetic is a guess dressed up and does not count as graded.

### Same countables, same number
Two graders holding the same citations produce the same rating. A disagreement over a number is a disagreement over the countables — resolve it by recounting the evidence, never by negotiating the number.

### State which elements are missing beside the rating
A claim missing any of the seven elements names which are missing beside its computed rating, per the evidence bar. The rating says how much evidence exists; the missing-element note tells the owner what a higher rating would need. Nothing is removed for a missing element — the rating and its note ride to the pick.

## 2. Measure markets by contract

Every market measurement is an observation with a contract. A measurement whose contract cannot be met is written "we do not know" — never "High", "strong", or "present" as an assertion.

- **Competitor density** — a counted, NAMED list: every seller named, with the boundary rule that decided who is in and who is out stated beside the list. The number is the length of the list; no list, no number.
- **Buying power** — observed prices actually paid: the price, what it was paid for, and the citation showing a real person paid it. Never an inference from who the buyers seem to be.
- **Already-paying** — named products these people currently pay for, each with a cited payer. A category ("they pay for tools") is not a measurement.
- **Ease-to-sell** — a read derived from the three measurements above, stated with the three values it derives from. It carries no independent number.

### Never let internal coherence raise a number
Plausible, coherent, or strategically attractive is not evidence. Only countables — cited elements, verified occurrences, fetched quotes, contract-met measurements — move a rating.
