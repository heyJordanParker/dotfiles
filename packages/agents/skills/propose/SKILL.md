---
name: propose
description: |
  Mandatory contract for every proposing-state turn — the classifier injects it on each one. Builds the Proposal as a Decision Hierarchy: dominant call on top, gated children nested beneath it, genuine peers side by side, the work breakdown tagged onto Decisions rather than used as the skeleton. Opens with a whole-change annotated map, shows code instead of prose about code, and carries the /pcc shape (pros, cons, confidence) on every Decision node, guarded by the seven named Proposal failures. TRIGGER on every proposing-state turn — the classifier mandates this. DO NOT TRIGGER for executing turns (that loads /execute) or auto turns (mixed intents resolve action first). For the pros/cons/confidence ranking, /pcc is canonical; for the opening maps, /show-architecture is canonical.
---

# Proposal

- A Proposal is the Agent's proposed path to a Goal, organized for Architect Review.
- The Proposal is the Decision Hierarchy: dominant Decision first, gated Decisions nested beneath it, genuine peers side by side.
- Slices, steps, and files tag onto Decisions; they never organize the Proposal.
- The cto Agent Prompt already covers Verification, no hedging, full reads, and regressions. This Skill adds the Proposal shape.

## 1. Write the shell

### Start at the title
The first character of the response is `#`. No setup sentence, read-summary, or preface comes before it.

### Write Why in three to five sentences
Say what the code does after this that it did not before, which dependency relationship changes, what contract that creates, and what is difficult. Use Domain words, not framework mechanism. Do not list upcoming Decisions.

### Use only the four Proposal sections
Write the Proposal in this order: title, Why, the whole change, Decisions. Do not add top-level sections.

### End at the work
End at the last work-tag, the last confidence number, or the last Decision node's last sentence. No closing sentence and no summary of the Plan's safety.

## 2. Build the Decision Hierarchy

### Put the dominant Decision first
The dominant Decision is the one that changes the most files, the widest dependency, or the largest cost swing.

### Nest gated Decisions under their parent
When one parent answer deletes a child Decision, the child nests under that parent. Heading depth mirrors gate depth.

### Keep genuine peers at the same level
Two Decisions are peers only when the Architect's answer to one does not change the meaning of the other.

### Show importance by position only
Importance and dependency are shown by hierarchy. Never write that one Decision is the most important or that one Decision gates another.

### Do not manufacture Decisions
Pure plumbing that carries no open choice lives only in the whole-change map. A settled mandate is the work, not a Decision.

Never:
  ```text
  The Plan, importance-ordered:

  1. Lazy variant generation — store original only, generate on first access.
  2. Hybrid R2 keys — {hash}-{slug}.webp.
  3. WP attachment sync — store_id on the media table.
  4. Specialized media_folders table.
  5. Dedup — hash before processing.
  ```

Example:
  ```text
  When are sized variants made?               lazy  vs  pre-generate  vs  Cloudflare
  └── How does a variant URL resolve?         redirect-to-R2  vs  proxy-bytes
  How does WP see a file with no attach row?  store_id  vs  join-table  vs  postmeta
  What is the R2 object key?                  hash+slug  vs  hash-only  vs  folder-path
  └── How does a folder move rewrite keys?    leave-keys  vs  rewrite-keys
  What table holds folders?                   specialized  vs  generic-typed
  Where is the dedup hash taken?              before-processing  vs  after-upload
  ```

Each row is one Decision: the question, then options separated by `vs`. Options are highest-confidence first. `vs` means the options are mutually exclusive. Indentation means gated.

## 3. Draw the maps

### Open with the whole-change map
Every Proposal opens with one whole-change annotated file tree in `/show-architecture` style: every file the change creates or touches, `<- (NEW)` on new files, and a three-to-five-word job note on each file.

### Open the Decisions section with the hierarchy map
The Decision Hierarchy map shows every Decision as one row, options separated by `vs`, options ordered highest-confidence first, and gated Decisions nested beneath their parent. The map carries candidates and rank only. The Decision node heading carries settled or open state.

IF a Decision changes file ownership, data movement, or dependencies:
### Add a scoped Decision map
Use a scoped file tree or a `/show-architecture` boxes-and-arrows map before that Decision's artifacts.

IF the Decision shape is obvious from its code block:
### Skip the decorative map
Do not add a map that helps no Review.

## 4. Show artifacts

### Show code instead of prose about code
Show concrete artifacts in fenced blocks: file paths, public API names and signatures, route strings, and database changes as data definition language. Prose is only for the Architectural WHY: why an Architecture edge sits here, why a dependency runs this direction, or why a difficulty is hard.

Never:
  > The MediaController gains a finalize endpoint that downloads the temporary object from R2, validates the MIME type and magic bytes, checks dimensions and size, strips EXIF, sanitizes any SVG, converts to WebP, re-uploads under the hybrid key, and creates the Media record. A new content_hash column stores the SHA-256, a key column holds the hybrid key, and store_id maps the WordPress attachment.

Example:
  ```text
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

Validation runs server-side after the presigned PUT because the browser cannot be trusted to enforce the 20MB / 20MP ingest ceiling. That is the only WHY the prose owes.

## 5. Write each Decision node

### Head each Decision with its question and state
Use a plain heading with no `Decide:`, `Fork:`, or `Choice:` prefix. End it with `(settled, NN%)` when the Agent broke the Architecture and one option won, or `(open — your call)` when the Architect must weigh Context the code cannot answer.

### Put node content in the fixed order
Each Decision node carries, in order: map if useful, artifacts, options in the /pcc shape, then one work-tag naming the files, methods, and Slice this Decision lands in.

Template:
  ```markdown
  **Option A — on the existing service.** What it is, concretely, in our code.

  - Precedent: the exact file or system this builds on, full path — or research proving none exists
  - pro: how it solves the stated problem
  - con: the real cost it adds, the one not seen until it bites
  - confidence: 82%

  **Option B — a new single-purpose class.** What it is, concretely.

  - Precedent: ...
  - pro: ...
  - con: the one cost that ruled it out
  - confidence: 55%
  ```

### Keep cons real
A con is a real cost the option adds. It is never a cross-option comparison, normal implementation effort dressed as a flaw, or Fluff to balance the Template. If an option has no real con, say so.

### Keep confidence differentiated
Confidences differ by more than 10 points. Clustered confidences mean the analysis is unfinished. Read more code; do not renumber.

IF the Decision is settled:
### Put the surviving option first
The surviving option appears in full: Precedent, pro, con, confidence. The options it beat follow at lower confidence, each carrying only the con that killed it. The survivor ships because it is most correct, never because it is smaller.

IF a settled Decision has no real alternative:
### Do not invent one
The survivor stands alone.

IF the Decision is open:
### Do not pick for the Architect
Open options are genuine peers, each in full, ordered highest-confidence first, with no direction named.

IF a parent Decision is open:
### Collapse its gated children
Name the gated children under the parent, but do not expand them with options or code until the parent is settled.

## 6. Protect requirements and questions

### Use Decision nodes only for real choices
Write a Decision node only when the brief left a real open choice and picking one option makes the work under the other wrong: different mechanisms, Architecture edges, data movement, or dependencies. The brief's mandate is the work, not a Decision. For audits, gaps, or errors, deliver findings with place and impact; do not manufacture options. When the Architect already picked an option and asks to refine one part, apply the refinement and nothing else.

### Place each Decision once
A Decision appears where it is made. Do not foreshadow it in Why, summarize it, or repeat it.

### Preserve every unanswered question
Every question the Architect did not answer reappears, in full, in every later Proposal until they answer it. Never drop a question because the Proposal moved on. Never assume an answer to keep going.

### Ask only external Context questions
Emit a question only for a real external Context gap: environment, prerequisite, requirement, or scope edge the code cannot answer. State what flips under each answer. Never invent an assumption to fill the slot. Never rephrase an option-pick as a question. If no real gap exists, write `No open questions.`

### Prevent the seven named Proposal failures
1. `vacuous-proposal` — Proposal shape, no Architectural Decision in it. Fix: every node names a concrete change and a real alternative.
2. `capability-loss` — the User can no longer do something, or the system can no longer do something. Every removal names what it removed and where the protected capability now lives. Backwards compatibility is not a capability.
3. `worse-option-shipped` — the work-tag points at an option the Agent knows is suboptimal. The work-tag points at the option the Agent believes most correct. Diff size is never the reason.
4. `requirement-drop` — a stated requirement is relaxed, narrowed, or deferred. Every requirement appears, met. A conflict is a Decision, never something the Agent resolves by dropping a requirement.
5. `contradiction-elision` — a conflict between requirements, or between a requirement and the code, is hidden. Surface it as the Architect's Decision.
6. `mixed-layer-pcc` — parent and child Decisions are flattened as peers. Gated Decisions nest, so a parent answer visibly deletes its children.
7. `hedged-proposal` — the Proposal says `likely`, `may`, `should` in the expected-behavior sense, `probably`, `might`, `could`, `perhaps`, `I would expect`, `in theory`, `it appears that`, or `it seems`. Open the file, read the function, and write what is. The one exception is a genuine stated unknown: `I have not checked X`.

## 7. Run the self-check

### Keep the response readable
Use full width, short sentences, one idea each, blank lines between ideas, and no paragraph over three sentences.

### Use actual words
No metaphor, jargon, hype, or importance in prose. Banned analogy words in a Proposal: `cutover`, `fork`, `harvest`, `leverage`, `surface` as a verb, `bridge`, `glue`, `wire up`, `hang off`, `ride on`, `load-bearing`, `safety net`.

### Figure options out before writing them
Run every option through the requirements before writing it. Drop any option that fails, or state it as rejected with the reason worked out. If you catch yourself writing `actually`, `wait`, `hmm`, or `X does not actually Y`, delete the option and rewrite with clean, pre-validated options.

### Do not echo the requirements
Never hand the requirements back reworded as the Plan. State the concrete change, code paths, and data movement.

### Rewrite on any failed check
Check the Decision Hierarchy is the skeleton, hierarchy is visible, maps are present, artifacts are shown, /pcc appears on every real Decision, none of the seven failures apply, and unanswered questions persist. If any check fails, rewrite before sending.
