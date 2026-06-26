---
name: propose
description: |
  Mandatory contract for every proposing-state turn. Loads automatically when the session state is proposing — the classifier injects "load /propose" on every proposing turn, and the rules here are the contract for what the agent emits. Builds the proposal as a decision tree — parent decision gating child, dominant call on top, siblings as genuine peers — with the work breakdown tagged onto decisions rather than used as the skeleton. Opens with a whole-change annotated map, shows code instead of prose about code, and carries the /pcc shape (pros, cons, confidence) on every decision node. Covers the seven named proposal failures (vacuous-proposal, capability-loss, worse-option-shipped, requirement-drop, contradiction-elision, mixed-layer-pcc, hedged-proposal) plus no-hedging, options-figured-out-first, unanswered-questions-persist, one-complete-response, no-requirement-echo. TRIGGER on every proposing-state turn — the classifier mandates this. DO NOT TRIGGER for executing turns (the agent is implementing, not proposing) or auto turns (mixed intents resolve action first). For the pros/cons/confidence ranking a decision node uses, /pcc is canonical; for the maps it opens with, /show-architecture is canonical.
---

# Proposal

The contract for every change proposal, in any codebase, for any problem. The classifier loads this skill on every proposing-state turn. The rules apply to the whole proposal, not just a section of it.

The architect reviews decisions all day. A flat list of same-level items forces them to rebuild — every proposal — which call gates which. This format does that work for them once. **The proposal is the decision hierarchy.** The architect reads it top-down: the dominant call first, its dependents nested beneath it, genuine peers side by side. Most of their corrections land on concrete artifacts — file paths, method and API names, schema changes — so those are shown for scanning, never buried in prose that describes code instead of showing it.

The quality bar is clean readable markdown at full width: a whole-change map, a visible decision tree, real code in fenced blocks, honest options. Every rule below removes a way proposals get worse.

The cto agent prompt covers verification, no hedging, full reads, no regression. The rules below add to those, never replace them.

## The decision tree is the proposal

Decisions are the skeleton. The work breakdown — slices, steps, files — tags onto each decision; it is never the organizing structure.

Build the tree the architect would otherwise build in their head:

- **Dominant call on top.** The decision that shapes the most surface — the most files, the widest dependency, the largest cost swing — is first.
- **Parent gates child.** When deciding the parent one way deletes the child decision entirely, the child nests under the parent. Heading depth mirrors gate depth.
- **Siblings are genuine peers.** Two decisions are peers only when the architect's answer to one does not change the meaning of the other. Peers sit at the same heading level.

Importance and dependency are shown by structure, never asserted in prose. Never write "the most important decision is" or "this gates that" — the position and nesting say it.

**Bad — flat, importance-ordered, same-level peers the architect must untangle:**

```
The plan, importance-ordered:

1. Lazy variant generation — store original only, generate on first request.
2. Hybrid R2 keys — {hash}-{slug}.webp.
3. WP attachment sync — store_id on the media table.
4. Specialized media_folders table.
5. Dedup — hash before processing.
```

Five items read as equal weight. Nothing tells the architect that item 1 gates the variant-addressing work, or that item 3 is independent of all the rest. They reconstruct the tree themselves.

**Good — the tree is built; the gate is visible:**

```
When are sized variants made?               lazy  vs  pre-generate  vs  Cloudflare
└── How does a variant URL resolve?         redirect-to-R2  vs  proxy-bytes
How does WP see a file with no attach row?  store_id  vs  join-table  vs  postmeta
What is the R2 object key?                  hash+slug  vs  hash-only  vs  folder-path
└── How does a folder move rewrite keys?    leave-keys  vs  rewrite-keys
What table holds folders?                   specialized  vs  generic-typed
Where is the dedup hash taken?              before-processing  vs  after-upload
```

Each row is one decision: the question, then its options separated by `vs`. The options are ordered highest-confidence first, so the leftmost is the call the agent leans to. `vs` marks them mutually exclusive — pick one.

Indentation is gating. Choosing Cloudflare on variant delivery deletes the variant-addressing row, so that row nests under it. The architect never reads work the top answer discards (mixed-layer-pcc, prevented structurally).

Whether each call is settled or still open is stated at its node, never on the map. The map carries the candidates and their rank — nothing else.

A decision node organizes work but is not itself plumbing. Pure plumbing that carries no open choice — the picker grid, the edit panel — lives in the whole-change map only. Never manufacture a decision node for it. Never present a settled mandate as a choice.

## Maps are mandatory

Every proposal opens with one whole-change annotated file tree, `/show-architecture` style: every file the change creates or touches, `<- (NEW)` on new ones, a 3-5 word role note on each. This is the architect's single view of the full surface before any decision.

The decision tree opens with a decision-hierarchy map — the indented tree above: every decision is one row, its options `vs`-separated and ordered highest-confidence first, gating shown by nesting. The whole structure and every option set is seen at a glance before any node is read. The map carries the candidates and their rank; the node heading carries whether the call is settled or open.

Each decision opens with its own map where it aids review: a scoped file tree for a decision that moves files, a relationship diagram (`/show-architecture` boxes-and-arrows) for a decision about how data moves or who owns what. A decision whose shape is obvious from its code block needs no map — never add a decorative one.

## Show code, not prose about code

Concrete artifacts are shown for scanning, in fenced blocks: real file paths (`app/Tenant/Media/MediaController.php`), method and public-API names with signatures, route strings, and database changes as DDL — tables, columns, indices. Prose is reserved for the architectural why: why a boundary sits here, why a dependency runs this direction, why a difficulty is hard. Prose never narrates what a code block already shows.

**Bad — prose describing code the architect must parse back into structure:**

> The MediaController gains a finalize endpoint that downloads the temporary object from R2, validates the MIME type and magic bytes, checks dimensions and size, strips EXIF, sanitizes any SVG, converts to WebP, re-uploads under the hybrid key, and creates the Media record. A new content_hash column stores the SHA-256, a key column holds the hybrid key, and store_id maps the WordPress attachment.

**Good — artifacts shown, prose carries only the why:**

```
POST /media/finalize  →  MediaController::finalize()  →  MediaService::ingest()

MediaService::ingest(string $tempKey, array $context): Media
  // validate(mime, magic-bytes, ≤5000×5000, ≤20MB) → stripExif → sanitizeSvg
  // → toWebP (skip GIF) → putObject({hash}-{slug}.webp) → deleteObject($tempKey)
```

```sql
ALTER TABLE media
  ADD content_hash char(64) NOT NULL,   -- SHA-256 of raw bytes, dedup key
  ADD key          varchar  NOT NULL,   -- hybrid R2 key {hash}-{slug}.webp
  ADD store_id     integer  UNIQUE;     -- WP attachment post id, nullable
CREATE INDEX media_content_hash_idx ON media (content_hash);
```

Validation runs server-side after the presigned PUT because the browser cannot be trusted to enforce the 20MB / 20MP ingest ceiling — the only why the prose owes.

## Each decision node

A decision node is the unit the tree is built from. Each one carries, in order:

1. **The question, as a plain heading, tagged with its state.** No "Decide:", "Fork:", "Choice:" prefix. The heading ends with `(settled, NN%)` when the agent broke the design and one option won, or `(open — your call)` when the architect must weigh context the code cannot answer. Nesting (peer or gated) is already set by the tree.
2. **A map**, where it aids review (see Maps).
3. **The artifacts in play**, shown in fenced blocks (see Show code).
4. **The options, in the /pcc shape** — two or more genuine alternatives, each with precedent, pro, con, confidence:

```
**Option A — on the existing service.** What it is, concretely, in our code.

- precedent: the exact file or system this builds on, full path — or research proving none exists
- pro: how it solves the stated problem
- con: the real cost it adds, the one not foreseen until it bites
- confidence: 82%

**Option B — a new single-purpose class.** What it is, concretely.

- precedent: ...
- pro: ...
- con: the one cost that ruled it out
- confidence: 55%
```

5. **The work it tags** — one line naming the files, methods, and slice this decision lands in. This is the work breakdown hanging off the decision, not the skeleton.

A con states a real cost the option adds — never a cross-reference to another option, never normal implementation effort dressed as a flaw, never filler to balance the format. If an option has no real con, say so. Confidences differ by more than 10 points; clustered confidences mean the analysis is unfinished — read more code, never renumber. No recommendation line, no "later steps assume A" — the shipped direction is the work-tag.

Every node is **settled** or **open**, and the heading says which.

**Settled** — the agent ran the options through the requirements and one survived. Name the surviving direction first, in full: precedent, pro, con, confidence. The options it beat follow, each at its own lower confidence, carrying only the one con that killed it. The survivor ships because it is most correct, never because it is smaller; the rejected options are shown so the architect can overturn the call in one glance, never as footnoted regret. If the call has no real alternative, the survivor stands alone — never invent one to fill the shape.

**Open** — the architect must weigh business or scope context the code cannot answer. The options are genuine peers, each in full, ordered highest-confidence first, no direction named. The agent never picks an open decision for the architect.

An open parent collapses its children. Name the gated children under it, but do not expand them — no options, no code — until the parent is settled. Expanding work the open answer might delete is the waste the gate exists to prevent.

## The shape of a proposal

In order, no other top-level sections:

1. **Title** — the first character of the response is `#`. No "I have what I need", no summary of what you read. A line of any kind before the title is a failure.
2. **Why** — three to five sentences. What our code does after this that it did not before, which dependency relationship changes, what new contract that creates, and what is genuinely difficult — in domain terms, not framework mechanism. No third-party internals. No list of upcoming decisions.
3. **The whole change** — the opening whole-change map.
4. **The decisions** — the decision-hierarchy map, then the decision nodes in tree order.

## The seven named failures

Every rejected proposal is one of these. Each has a name so the agent catches itself and the architect names what they reject.

1. **vacuous-proposal** — proposal shape, no architectural decision in it. Restating the brief in tree layout, listing files read, decision nodes shaped like "investigate / consider / evaluate". Fix: every node names a concrete change and a real alternative.
2. **capability-loss** — silently regressing a capability (user can no longer X, system can no longer Y). Every removal names what it removed and where the protected capability now lives. Backwards compatibility is not a capability.
3. **worse-option-shipped** — tagging the work to an option the agent knows is suboptimal, sometimes with a footnote pointing at the better one. The work-tag points at the option the agent believes most correct. Diff size is never the reason.
4. **requirement-drop** — a stated requirement silently relaxed, narrowed, or deferred. Every requirement appears, met. A conflict is surfaced as a decision, never resolved by dropping the requirement.
5. **contradiction-elision** — silently papering over a conflict between requirements, or between a requirement and what the code does. Surface it as the architect's decision; the agent does not pick which requirement wins.
6. **mixed-layer-pcc** — batching decisions where a parent answer obliterates a child. Prevented structurally here: gated decisions nest, so a parent's answer visibly deletes its children. Never flatten a gate into peers.
7. **hedged-proposal** — "likely", "may", "should" (expected-behavior sense), "probably", "might", "could", "perhaps", "I would expect", "in theory", "it appears that", "it seems". These are confessions that the code was not read. **Banned in any proposal.** Open the file, read the function, write what is. The one exception is a genuine, stated unknown: "I have not checked X" is correct when X was not read.

## What is and is not a decision

A decision is a point where the brief left a real open choice and picking one option makes the work under the other wrong — fundamentally different mechanisms, boundaries, data flows, or dependencies. The brief's own mandate is never a decision; a change the brief asks for is the work, not a question. When the architect asked for an audit, gaps, or errors, deliver the findings each with its place and impact — never manufacture options. When the architect already picked an option and asks to refine one part, apply the refinement and nothing else.

A decision appears once, where it is made — never foreshadowed in the Why, never summarized, never repeated.

## Unanswered questions never disappear

Every question the architect did not answer reappears, in full, in every later version until they answer it. Never drop a question because the proposal moved on. Never assume an answer to keep going.

Emit a question only for a real external-context gap — an environment, prerequisite, constraint, or scope boundary the code cannot answer — and state what flips under each answer. Never invent an assumption to fill the slot. Never rephrase an option-pick as a question. If no real gap exists, write "No open questions."

## No metaphor. No jargon. No hype. No importance-in-prose.

Every word names the actual thing. Banned analogy-words: "cutover", "fork", "harvest", "leverage", "surface" (verb), "bridge", "glue", "wire up", "hang off", "ride on", "load-bearing", "safety net". Say the action. Importance is carried by tree position, never stated — never "the genuinely hard part", "the key one", "the biggest lever".

## Readability

Full width. Short sentences, one idea each. Blank line between ideas. No paragraph over three sentences. Visible whitespace.

## One response, complete

Deliver the entire proposal in one response. No progressive disclosure. As it evolves, prune resolved and superseded nodes and re-emit the live tree — never an "everything else unchanged" handoff.

## Options are figured out before they are written

Run every option through the requirements yourself before writing a word of it. An option that fails a requirement is dropped or stated as rejected with the reason worked out. Never discover a flaw mid-sentence. If you catch yourself writing "actually", "wait", "hmm", or "X does not actually Y", that option was not figured out — delete it and rewrite with clean, pre-validated options.

## Don't echo the requirements

Never hand the requirements back reworded as the plan. State the concrete change, the code paths, the data flow.

## How it ends

Ends at the last work-tag, the last confidence number, or the last node's last sentence. No closing sentence, no summary of the plan's safety.

## Self-check before sending

1. **decisions-as-skeleton** — the structure is the decision tree; slices and files tag onto decisions, never organize the proposal.
2. **visible hierarchy** — dominant on top, gated decisions nested, peers at one level; importance shown by position, never asserted.
3. **maps present** — a whole-change map opens; the decision tree opens with a hierarchy map; each decision that needs one has its own.
4. **artifacts shown** — file paths, signatures, route strings, DDL in fenced blocks; prose only for the why.
5. **/pcc on every decision** — precedent, pro, con, confidence; confidences differ by >10 points. Settled nodes lead with the surviving option and show what it beat at lower confidence; open nodes show peers with no direction named; neither invents an option to fill the shape.
6. **the seven failures** — none apply.
7. **questions persist** — every unanswered question is still here, or "No open questions".

If any check fails, rewrite before sending.
