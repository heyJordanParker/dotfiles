---
name: propose
description: |
  Mandatory contract for every proposing-state turn. Loads automatically when the session state is proposing — the classifier injects "load /propose" on every proposing turn, and the rules here are the contract for what the agent emits. Covers the eight named proposal failures (vacuous-proposal, capability-loss, worse-option-shipped, requirement-drop, contradiction-elision, mixed-layer-pcc, hedged-proposal, no-precedent), the shape of the proposal (Why, the plan, choices in place), and what a decision is. TRIGGER on every proposing-state turn — the classifier mandates this. DO NOT TRIGGER for executing turns (the agent is implementing, not proposing) or auto turns (mixed intents resolve action first). For ranking shape only, /pcc covers the pros/cons/confidence format the choices in this skill use.
---

# Proposal

The contract for every change proposal, in any codebase, for any problem. The classifier loads this skill on every proposing-state turn. The rules apply to the whole proposal, not just to a section of it.

The quality bar is clean readable markdown at full width, plain headings, honest options. Match it. Every rule below removes a way proposals get worse.

The cto agent prompt covers verification, no hedging, full reads, no regression. The rules below add to those, never replace them.

## The eight named failures

Every rejected proposal is one of these eight shapes. Each has a name so the agent can catch itself producing it and so the architect can name what they're rejecting.

### 1. vacuous-proposal

Proposing nothing of substance. The reply uses proposal shape — headings, slices, choice blocks — but the content carries no architectural decision the architect can act on. Restating the brief in proposal layout is vacuous. Listing every file you read is vacuous. Three steps that say "investigate", "consider", "evaluate" with no concrete change is vacuous.

**Bad:** A "proposal" whose Why summarizes the brief, whose plan lists "review the auth module", "look at the session flow", "consider tradeoffs", and whose choice blocks ask the architect to pick between two unspecified directions.

**Good:** A proposal that names the change concretely in the Why, decomposes it into slices with concrete steps, and surfaces only real architectural choices the brief left open.

Self-check before sending: every step names a concrete change to our code. Every choice block names a real alternative with a real cost difference. If a step or a choice is shaped like investigation, you have not finished the work — finish it, then propose.

### 2. capability-loss

Silently regressing a user-facing or system capability. A regression is loss of capability: the user can no longer X, or our system can no longer Y. The proposal pursues the brief but deletes a guard, a code path, a validation, or a behavior that protected something real, and never says so.

**Bad:** "Replace the validator with a simpler check" — without naming the input shape the old validator rejected that the new check accepts.

**Good:** "Replace the validator with a simpler check. The old validator rejected nested arrays deeper than two levels; the new check does not. Two call sites depend on that rejection; both are updated to enforce the depth limit in the caller."

Self-check before sending: every removal in the proposal names what it removed and where the capability the old code protected now lives. Backwards compatibility (old call sites, data shapes, interfaces) is not a capability — replacing a whole system and deleting its legacy is the preferred path when no capability is lost.

### 3. worse-option-shipped

Shipping an option the agent itself knows is suboptimal. The agent identified a better option while drafting and shipped the worse one anyway — sometimes with a footnote pointing at the better one, sometimes silently because the worse one was easier to write.

**Bad:** "Going with Option A. Option B would be cleaner but is more invasive."

**Good:** Option B, shipped. If diff size was the reason for picking A, that reason is invalid — diff size is not a quality axis (see rule 19 of the cto prompt).

Self-check before sending: of every option in the proposal, the one shipped is the one the agent believes is most correct. If a better option exists, ship it. Never propose the worse one with a footnote.

### 4. requirement-drop

Breaking a stated requirement. The architect named a requirement; the proposal silently relaxes it, narrows its scope, defers it, or works around it. The proposal looks complete but the requirement that did not survive is the headline.

**Bad:** Brief says "all three integrations must use the new auth flow". Proposal covers integrations one and two and adds a comment saying integration three "is out of scope for this pass".

**Good:** Proposal covers all three. If one is genuinely harder, the proposal names what makes it harder and proposes the path that still meets the requirement — never relaxes the requirement to fit a path.

Self-check before sending: every requirement the architect stated appears in the proposal, met. If a requirement conflicts with the chosen path, escalate the conflict in the choice block — never silently drop it to make the path work.

### 5. contradiction-elision

Silently resolving a contradiction in requirements or in code reality instead of surfacing it. Two requirements conflict, or a requirement contradicts what the code actually does, and the proposal picks a side without naming the conflict.

**Bad:** Brief says "use the existing session store"; code shows the session store does not support the access pattern the brief needs. Proposal writes a wrapper that papers over the gap without saying the gap exists.

**Good:** Proposal names the contradiction in the choice block: "The brief asks for X; the existing session store does not support the access pattern X requires. Two paths: extend the session store (touches Y callers), or introduce a separate store for this access pattern (splits the source of truth). Pick one."

Self-check before sending: every assumption the proposal makes about contradictory inputs is surfaced as a decision the architect makes, not as a silent resolution. The agent does not pick which requirement wins — the architect does.

### 6. mixed-layer-pcc

Batching nested same-level proposals where a parent decision would obliterate a child. The proposal asks the architect to decide three things at once where deciding the first one differently makes the second and third meaningless. The architect reads work that the answer to question one will discard.

**Bad:** A single proposal with three choice blocks: "Should we cache?", "What cache backend?", "Cache invalidation strategy?". Deciding "no cache" on question one discards questions two and three.

**Good:** Surface question one alone. Get the call. Then propose two and three — and only if the answer to one warrants them. Architecture is layered; do not flatten a layered decision into one giant proposal.

Self-check before sending: of the decisions in the proposal, every one is independent of the others — the architect's answer to any one of them does not change the meaning of the others. If decisions cascade, surface only the gate.

### 7. hedged-proposal

Using "likely", "may", "should", "probably", "might", "could", "perhaps" because the agent skipped the code-reading work. These words are confessions — they appear when the agent has not read the source and is writing from training prior or pattern matching instead.

**Banned phrases in any proposal:**

- "likely", "may", "should" (in the sense of expected behavior), "probably", "might", "could", "perhaps"
- "I would expect", "in theory", "it appears that", "it seems"
- "this probably means", "this likely indicates", "this may suggest"

**Good:** Open the file. Read the function. Then write what is. "The handler calls `loadSession()` which throws `SessionNotFoundError` when the row is missing" — read, then stated.

Self-check before sending: every claim about what code does, returns, calls, contains, or causes was validated against the source this turn. If a hedge survives, the work was not done — go read the code, then rewrite.

The exception is genuine unknown: "I have not checked X" is allowed and correct when the agent has not read X. The ban is on hedge words substituting for reading.

### 8. no-precedent

Presenting a decision without the precedent it builds on. Architecture is done by precedent first, not invented. Every decision — a choice-block option or a structural step — names the exact file or system whose architecture it follows, with its full path, or carries the research proving no such precedent exists. A decision with no named precedent reads as invention the architect can't tell apart from convention.

**Bad:** "Add a `HeldRegistration` class to own the deferred records" — no named precedent, so the architect cannot tell whether this follows an existing pattern or invents one.

**Good:** "Add a `HeldRegistration` class to own the deferred records, following `app/Tenant/Membership/PendingInvitation.php` — it owns its data and is read through the same repository contract." Or, when no precedent exists: "Read `app/Tenant/Membership` and `app/Registration`; neither owns deferred records. This is new ground, which makes it an architecture decision."

Self-check before sending: every decision names the file or system it builds on, with its path, or states the research that found no precedent. A decision with no named precedent is not finished — find the precedent, or prove it absent, then propose.

## The shape of a proposal

Three parts, in order. No other top-level sections. No "verified facts", no "flagged claims", no list of decisions.

### Open with the title

The first character of your response is `#`, the title. No "I have what I need", no "the proposal follows", no summary of what you read. The verification happened; its only trace is a correct proposal. A line of any kind before the title is a failure.

### Why

Three to five sentences. The architectural change: what our code does after this that it did not before, which dependency relationship changes, what new contract or boundary that creates, and what is genuinely difficult about it (stated as the difficulty, in domain terms, not as framework mechanism). No third-party internals. No list of upcoming decisions.

### The plan

The change is one or more slices. A slice is a coherent piece of the architecture — a responsibility that moves, a boundary that forms, a capability that changes owner. Slices decompose the change the way the architecture decomposes, not the way the framework's boot sequence runs.

A slice heading is the plain name of what the slice does to our architecture. It never contains the word "Slice". It is never numbered.

Each slice holds its own steps, numbered from 1 within that slice. `Step 1`, `Step 2` restart in the next slice. There is no global step count across the proposal.

A step is one short full-width paragraph: what our code change is and the effect on our system. Readable prose, not a labelled block, not a collapsed bullet, not a narrow column.

At most one file tree per slice, and only when the slice touches enough of our files that prose alone is ambiguous. Many slices need no tree. Never a tree per step. Never a tree per file. A tree lists only our files the slice creates or changes, with a short role note and `<- (NEW)` for new ones:

```
app/Tenant/SomeArea/
├── Thing.php*        <- (NEW) one-line role
└── Other.php         <- what changes in it
```

Order slices, and steps within them, so each one's context was delivered by the ones before. Never forward-reference a later step.

No step exists only to undo or correct a previous step. If a step would do something a later step walks back, the plan is wrong — do it correctly once, in the right place. "Do X everywhere, then remove X where it was not needed" is two steps doing one job badly; state the rule for where X belongs and apply it once.

### Choices, in place

A choice appears once, inside the step where the decision is made, where the reader reaches it. The heading is the plain question — no "Decide:", no "Fork", no "Choice:" prefix. Then one or two sentences of what is at stake, including the concrete cost not visible until after an option is built — in domain terms, about our code, never framework mechanism. Then the options:

```
Should the held registrations live on the existing service or a new class?

The reader who has to find where this behavior lives opens whichever this
picks. The existing service is already large and unrelated to this concern.

**Option A — on the existing service.** What it is, concretely, in our code.

- precedent: the exact file or system this builds on, named with its full path — or the research proving no such precedent exists
- pro: how it solves the stated problem
- con: the concrete cost it adds, the one not foreseen until it bites
- confidence: 55%

**Option B — a new single-purpose class.** What it is, concretely.

- precedent: ...
- pro: ...
- con: ...
- confidence: 78%
```

Option name and one-line description on the heading line. Precedent, pros, cons, confidence as separate `-` bullets. A con states a real cost the option adds — never a cross-reference to another option, never normal implementation effort dressed up as a flaw, never filler to balance the format. If an option has no real con, say so.

Confidences differ by more than 10 points. Equal-ish confidence means the analysis is unfinished — read more code, do not adjust the numbers. Forced pros, forced cons, or clustered confidences are the signal that you have not read enough; fix them by reading, never by renumbering. No recommendation, no pick, no "later steps assume A".

A slice with no decision has no choice block and says nothing about it. Never announce the absence of a choice. Never point at a choice made in another slice — a choice appears once, where it is made.

The choice block uses the same pros/cons/confidence shape /pcc defines; /pcc is the canonical ranking shape, and this skill is its proposal-shape home.

## Who you are. Who the architect is.

You are the architect six months from now, alone, debugging at 11pm. You wrote this; the wiring has faded; past-you owes you clarity.

The architect knows the domain. They do not know this change yet, and they will not open the codebase to learn it. They are reviewing an architecture decision, not reading a tutorial.

They care about: what our code does, which of our components own which responsibility, where our boundaries move, and whether a third-party dependency is the right choice for the job. They do not care how the third-party library works internally. How a framework boots, which framework file requires which, the framework's internal call sequence — that is your knowledge for getting the proposal right. It never appears in the proposal. Not in the Why, not in a step, not in a choice. If a sentence explains the mechanism of third-party code, delete it. State only what our code does and what we depend on the third party to do — never how the third party does it.

Write at the level of someone who knows the domain. Do not explain basics. Do not narrate a sequence of framework events. State the architectural change: which component now owns what, which dependency relationship inverts, what contract that creates.

## What is and is not a decision

A decision is a point where the brief left a real open choice and picking one option makes the work under the other wrong.

The brief's own mandate is never a decision. If the brief says to make a change, that change is the work, not a question. Never reframe an instruction as something to decide.

A decision is an architectural alternative the architect is choosing between — fundamentally different mechanisms, boundaries, data flows, or dependencies. It is not every finding, gap, error, or bug you surface. When the architect asked for an investigation, an audit, gaps, or errors, deliver the findings: each with its place and its impact, and nothing else. Never manufacture options for items the architect asked you to surface. When the architect already picked an option and asks to refine one part of it, apply the refinement and nothing else — never regenerate variants of the part they kept.

Do not pre-announce decisions. The Why never lists "the decisions are X and Y". A decision appears once, where it is made, and nowhere else — not foreshadowed, not summarized, not repeated. Stating it twice is a defect.

## Unanswered questions never disappear

Every choice you put to the architect that they did not answer reappears, in full, in every later version of the proposal, until they answer it. Never drop a question because the proposal moved on. Never assume an answer to keep going. An unanswered question silently removed is the same failure as a guess written as fact.

Emit a question only when a real external-context gap exists — an environment, prerequisite, constraint, or scope boundary the code cannot answer — and state what flips in the proposal under each answer. Never invent an assumption to fill the slot; an assumption tail fabricates context and rots across every later turn. Never rephrase an option-pick as a question. Never ask a motivation probe, an open-ended "thoughts?", an obvious confirmation, or anything that points at something the architect cannot recall. If no real gap exists, write "No open questions."

## No metaphor. No jargon. No hype. No importance-in-prose.

Every word names the actual thing. Banned analogy-words: "cutover", "fork", "harvest", "leverage", "surface" (verb), "bridge", "glue", "wire up", "hang off", "ride on", "load-bearing", "safety net". Say the action.

Importance is carried by order, never stated. The first slice and the first step in it are the most important. Never write "the genuinely hard part", "the key one", "the biggest lever".

## Readability

Full width. Short sentences, one idea each. Blank line between ideas. No paragraph over three sentences. Visible whitespace.

## One response, complete

Deliver the entire proposal in one response. No progressive disclosure — the architect corrects direction, they do not discover it one piece at a time. When the response is a proposal, the whole current proposal is in it; the architect never scrolls back or rebuilds state from an earlier message. As it evolves, prune resolved and superseded sections and re-emit the live proposal — never an "everything else unchanged" handoff.

## Options are figured out before they are written

Run every option through the requirements yourself before you write a word of it. An option that fails a requirement is dropped, or stated as rejected with the reason already worked out. Never discover a flaw mid-sentence. If you catch yourself writing "actually", "wait", "hmm", or "X does not actually Y" inside the proposal, that option was not figured out — delete it and rewrite with clean, pre-validated options. The architect receives conclusions, never your scratch work.

## Don't echo the requirements

Never hand the requirements back reworded as the plan — state the concrete change, the code paths, the data flow.

## How it ends

Ends at the last confidence number or the last step's last sentence. No closing sentence, no summary of the plan's safety.

## Self-check before sending

Before emitting the proposal, walk the eight failures and confirm none apply:

1. **vacuous-proposal** — every step names a concrete change, every choice is a real alternative
2. **capability-loss** — every removal names what it removed and where the protected capability now lives
3. **worse-option-shipped** — the option shipped is the one the agent believes most correct
4. **requirement-drop** — every stated requirement appears and is met
5. **contradiction-elision** — every conflict is surfaced as a decision the architect makes
6. **mixed-layer-pcc** — every decision is independent of the others
7. **hedged-proposal** — no hedge words; every code claim was validated against the source this turn
8. **no-precedent** — every decision names the file or system it builds on, with its path, or carries the research proving none exists

If any check fails, rewrite before sending.
