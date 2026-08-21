---
name: show-me
description: Show the work in the reply — annotated file trees, relationship diagrams, state transitions, call and gate chains, timelines, because-chains, bars, signatures, column trees, and exact diffs, with the prose around them carrying the WHY. TRIGGER on every reply that explains code, Architecture, a change, a name, or a finding, and whenever the Architect says "show me". DO NOT TRIGGER for the pros-cons-confidence shape (that is /pcc) or the Decision Hierarchy (that is /propose).
---

# Show Me

- One goal: less text for the Architect to read, and more meaning in what stays.
- A view earns its place by carrying more than the paragraph it replaces, in fewer words.
- Prose about a shape that can be drawn is the failure this Skill corrects.
- The narrowest view that answers the question is the right one.
- Every view lives in the reply; this Skill writes no files.

## 1. Choose the view

### Use a file tree for location and change shape
A file tree shows where code lives, which files change, and how nearby files relate.
Example: use a file tree when explaining what exists and what changes.

### Use a relationship diagram for runtime behavior
A relationship diagram shows movement, ownership, or end-to-end behavior.
Example: use a relationship diagram when the question is how data moves through the system.

### Use a call flow for order
A call flow shows what runs, in what order, at what depth.
Example: use a call flow when the question is what happens first, or where a run breaks.

### Use signatures for a public surface
Signatures show what a caller writes. Names, parameters, and return types answer an API question that prose only paraphrases.

### Use a column tree for a database change
A column tree shows a table's columns, their types, and why each one exists.

### Use a gate chain for conditional rules
A gate chain shows which checks run, in which order, and where each one exits.

### Use a timeline for events across a period
A timeline shows when things happened and how far apart they sat.

### Use a because-chain for a root cause
A because-chain runs from the problem down to the Architecture that produced it.

### Use bars for any count, size, or complexity number
Bars show ranking and proportion before a single number is read.

### Use a table only when rows and columns both mean something
Two axes crossing, one short mark or word per cell: `○`/`●` state, `yes`/`no`, a count, or options separated by `vs`. The information is the match between row, column, and cell, so every axis carries equal weight.
Example:
  ```
                   Claude   codex
  skills inline      ●        ●
  hooks wired        ●        ○
  rules loaded       ●        ○
  ```
Never: a sentence in a cell, one column carrying more text than the rest combined, or a finding list re-sorted into a grid when nesting carries it.

### Show the highest-leverage view that answers
Views rank by altitude, and Architecture sits above code: a tree, diagram, signature, or chain answers an Architectural question, and code lines answer a code question. Show the highest view that carries the answer whole. Drop to code lines when the Decision lives in the exact lines — a defect in them, a subtle behavior no higher view carries, or wording under review.
Never: code lines answering a question a higher view already answers, or code quoted as proof that work happened.

IF the reply compares two or more options:
### Run /pcc before writing the options
/pcc filters the options, ranks confidence, and owns the one option format. Its filter can leave one option, and then the reply presents it directly with no options block.

### Use markers only when the symbol carries meaning
`○` and `●` carry checklist state, `↔` carries a relationship where both sides affect each other, and `=`, `≠`, `≈`, `≤`, `≥`, `±`, `×` carry a relation or formula. No other symbol earns a place.
Example: `● schema migration applied`; `○ end-to-end test passing`; `CheckoutView ↔ CartService`; `tax = subtotal × 0.2`.
Never: scatter symbols through prose as decoration.

## 2. Draw file trees when location matters

### Use box-drawing characters and short annotations
Use `├──`, `└──`, and `│` for structure. Use `<-` annotations under nine words, adapted to the purpose.
Template:
  ```
  directory/
  ├── file.ts*             <- annotation (under nine words)
  ├── subdirectory/
  │   ├── nested.ts*       <- changed file marked with *
  │   └── related.ts       <- context file (no *)
  └── context.ts
  ```

### Mark changed files with `*`
A changed file gets `*` suffix. An unchanged file gets a plain role annotation and no status prefix. Skip irrelevant files entirely.
Never: `KEEP:`, `REMOVE:`, `PRESERVE:`, `* new`, or `existing,` prefixes.

### Move an annotation past nine words out of the tree
A row keeps its name and a note under nine words. What the note cannot hold goes to the Decision that owns it, as a diff, a signature, or one prose sentence above the tree.
Never: a tree row wrapped onto a second line, or a note with a semicolon in it.

### Match annotations to the purpose
Overview annotations name responsibility. Feature annotations name data movement. Debugging annotations name dependency or failure location. When the row's name already carries that, the annotation carries the reason instead — why the file exists, why it changed, why it stays.
Example: `engine.ts* <- orchestrates subsystems`; `validate.ts* <- the browser cannot be trusted with the ceiling`; `UserRepo.ts* <- fails here`.
Never: annotations that repeat the filename or a tree with no annotations.

### List a file's methods under it when the file is the subject
One file under review is the same tree, one level deeper. The file's annotation says why it exists, its symbols indent beneath it with a job note each, and the signature under discussion sits inline. However large the file, the tree holds names and jobs only. Expand one region into its handlers when the question is about that region.
Example:
  ```
  admin/components/canvas/
  └── NodeCanvas.tsx              <- the graph the customer drags nodes around in
      ├── NodeCanvas()            <- the export every caller uses
      ├── NodeCanvasInner()       <- owns every ref, hook, and handler
      │   ├── handleNodesChange   <- collects dimensions, lays out once all are in
      │   ├── handleMove          <- holds an optimistic position until the move commits
      │   └── handleToggleGroup   <- collapses by rewriting the graph, not hiding nodes
      ├── toReactFlowGraph(graph, positions): { nodes, edges }
      ├── NodeCanvasToolbar()     <- zoom, fit, retry layout
      └── CanvasErrorPanel()      <- shown when a node kind has no renderer
  ```
Never: a separate card format, line numbers, internals, or two sections listing the same symbol twice.

## 3. Draw relationship diagrams when runtime behavior matters

### Put only touched fields and methods inside boxes
The box title is the component. Inside, list only fields or methods touched by the behavior. Mark the authoritative box.
Example:
  ```
  ┌─────────────────────────────────┐
  │ Cart  (source of truth)         │
  │ items: offer_id, product_id, qty│
  │ subtotal / tax / total          │
  └──────┬───────────────────┬──────┘
         │ read              │ read
         ▼                   ▼
  ┌──────────────┐   ┌──────────────────┐
  │ CheckoutView │   │ StoreService     │
  │ customer sees│   │ projectCartToWc()│
  └──────────────┘   └────────┬─────────┘
                              │ write
                              ▼
                     ┌──────────────────┐
                     │ WC_Order         │
                     └──────────────────┘
  ```

### Label every arrow with the relationship
Arrow labels name the behavior: `read`, `write`, `charge`, `emit`, or `call`. Run top to bottom from entry to terminal effect.
Never: unlabeled arrows.

### Show only the path being explained
The diagram is not an inventory. Include only boxes on the path that answers the question.
Never: every related component in the system.

### Show the full signature when one to three nodes are on screen
Few nodes leave room for the whole signature, which is the reason to draw them. Wrap a long signature across lines.
Never: truncating a signature with an ellipsis.

### Draw states and the events between them when a thing changes state
Each arrow carries the event that causes the move, so a transition that does not exist is visible by its absence.
Example:
  ```
  started ──run──> running ──answer written──> ok
                      │
                      ├──non-zero exit──> failed
                      └──cancel────────> cancelled
  ```
Never: a state diagram for a lifecycle with one path, where the call flow already answers it.

## 4. Draw chains when order or cause matters

### Group the calls into named phases
Separate phases with a blank line, keep one call per line, and put the note on the right. Indentation carries depth.
Example:
  ```
  start
    _resolve_agent          roster first, shared second
    definition_path         reads memory, model, effort

  run
    thread/start            prompt sent inline as baseInstructions
      events                -> <job>.jsonl
      answer                -> <job>.txt

  finish
    job record              -> codex-run-<job>.json
  ```
Never: stamping depth numbers or line numbers down the left side.

IF the call flow is itself a change:
### Move it into a diff block so the terminal colours it
The same flow inside a ```diff fence renders the changed lines in colour and leaves the rest grey.

### Draw a gate chain as a path with a labelled exit per check
Each check sits on the path in the order it runs, and every branch ends in its outcome.
Example:
  ```
  command ──not a write──> runs
     │
     └──is a write──> mode build ──yes──> runs
                          │
                          └──no──> refused
  ```
Never: conditional rules as a grid, which destroys the order that gives them meaning.

### Give a timeline one kind of stamp for its whole first column
Pick the stamp by span: a full date for a timeline crossing days, a clock time when every event sits inside one known day, an age when the reader cares how long ago. One kind per timeline.
Example:
  ```
  2026-08-13 19:31 ├ both skill directories renamed
  2026-08-13 19:36 ├ sync regenerates fourteen codex artifacts
  2026-08-14 00:23 └ two-column form rejected
  ```
Never: two kinds of stamp in one column, or an age appended to a line that already carries a clock time.

### Run a because-chain from the problem down to the Architecture
The first line is the problem in the words it was given. Every line under it opens with `└─ because`, sits two spaces deeper than the line it explains, and names the mechanism that produced it. No line ends with a full stop. /5-whys owns the Process that finds each answer.
Example:
  ```
  Platform code is supposed to stay out of WordPress, but something in provisioning reaches into it
    └─ because `CreateTenant` constructs `InstallTenant`, which invokes `WordPressService::installTenant` (commit e151c713)
      └─ because `CreateTenant` owns the install-and-configure dispatch instead of only the Platform tenant lifecycle
        └─ because no public contract lets Platform request tenant setup without importing Tenant jobs
  ```
Never: a bullet list, a numbered list, a code fence around the chain, a path or tooling detail in the first line, or a chain ending at a person, a habit, or "it was never done properly".

### Answer the question in the terms it was asked
Something missing is answered with where it is now, named as a place: the such-and-such table, the file by the name the project uses, the service by its name.
Example: `because migration 0149 moved them to the integrations table`.
Never: blame language such as "because commit 6dd00f39 did it", or a line naming a change without naming where the thing went.

### Name the destination and stop there
A move already says the old copy is gone.
Never: "then removed", "then cleared", "and deletes the old", or any clause about the source after the destination is named.

### Refer back to the subject instead of re-listing it
Example: `moved them to the integrations table`.
Never: "moved their configuration and secrets" when the line above already named them.

### Name each thing once, the way the project says it
The whole name sits inside one pair of backticks, including any leading word, so it reads as `migration 0149` and never as migration `0149`. Write a method the way the language writes it.
Never: a full file path, a line number, a list of methods or properties, or the name repeated at the end of the line.

### Carry the commit id on the line where that design was chosen
That line ends with `(commit 6dd00f39)`, or `(uncommitted)`. Every other line ends bare.
Never: the commit message, or a commit id on a line whose design was chosen elsewhere.

### Let the case set the depth
One line under the problem is a complete chain when one line answers the question. Write a further line only when it carries a fact the line above does not imply.
Never: a line restating the consequence of the line above it.

### Close every loop inside its own line
Say what the thing does.
Never: "hands over", "handles", "manages", "takes care of", "deals with", "is responsible for".

## 5. Show quantity as bars

### Draw bars for counts, sizes, and complexity
The label sits left, the bar is built from `█` and `▌`, and the value follows it. Order the rows by size.
Example:
  ```
  Rust        ████████████████████  43,679
  JavaScript  █████████████████     38,120
  Markdown    ██████████            21,763
  Shell       ▌                      1,448
  ```
Never: bars mixing two units in one chart, where the lengths compare nothing.

IF a row carries a change:
### Put the delta last and colour the row through a diff fence
Colour is per line, so a `+` row renders green, a `-` row renders red, and an unchanged row keeps two leading spaces and stays grey. The delta is optional and always sits last.
Example:
  ```diff
    Rust        ████████████████████  43,679
  + Markdown    ██████████            21,763   +2,562
  - Python      █████████             19,388     −310
  ```

### Draw a threshold line when a value is measured against a limit
The line carries the scale, the limit, and the value, so the answer is whether it is over rather than what the number is.
Example:
  ```
  complexity   0 ──────── p95 75 ────────────────────────── 355  NodeCanvas.tsx
  ```

## 6. Show code lines when the Decision lives in the lines

### Show two or three annotated lines, never the file
Quote only the lines that carry the behavior, with a `<-` note on each.
Example:
  ```python
  if not t or len(t) > 4000:   <- silently drops every long message
      continue
  if PAT.search(t):            <- keyword match, not a correction check
  ```
Never: pasting a whole file, or describing the lines instead of quoting them.

### Show the neighbours for any name under review
A name is judged against its siblings, because the siblings are the Precedent. Show the folder around a filename, the table around a column, the class around a method.
Example:
  ```python
  # packages/agents/hooks/lib/session_mode.py
  def is_dispatched(event: dict) -> bool     # sibling, sets the is_ shape
  def permits(mode: str, tool: str) -> bool  # sibling, verb + subject
  def resolve(event: dict) -> str            # the one under review
  ```
Never: one signature alone when the question is what to call it.

### Draw a column tree for every database change
The table heads the tree and its columns indent under it. The note says why the column exists, the type sits in its own column, and changed columns take `*`. Unchanged columns stay listed with no note, so a removal has context around it. Indexes and keys appear only when the change touches them.
Example:
  ```
  media                              one row per stored file
  ├── content_hash*  char(64)        the same upload never stores twice
  ├── key*           string unique   addresses the object in the bucket
  ├── folder_id      bigint null     a file can sit in a folder, or loose
  ├── metadata       jsonb           per-type detail stays out of columns
  ├── filename       string
  └── mime_type      string
  ```
Never: a data definition statement pasted in place of the tree, or two column lists side by side.

## 7. Diff every change

### Diff the exact text of a Prompt change
A Prompt change always earns the code block, because the wording is the Decision. Show the removed lines and the added lines verbatim.
Never: summarizing the edit, paraphrasing the new wording, or naming the file and describing what happens to it.

### Diff the structure the change lives in
The `+` and `-` lines work on any structure, not only file text. Diff a call flow, a file tree, a column tree, or a step list the same way.
Example:
  ```diff
    1. Choose the view
       Use a file tree for location and change shape
  +    Use a call flow for order
  - 4. Use narrow markers
  + 4. Show code instead of prose about code
  ```

### Change only the lines that changed
Every untouched line stays byte-identical, so the eye lands on the change.
Never: reformatting, re-wrapping, or re-ordering the lines around the edit.

### Show one diff per subject
For text, a tree, or a step list, one diff carries both states. Two blocks side by side make the reader hold four columns at once.
Never: a before block followed by an after block for a change one diff can carry.

### Draw architecture before and after as two diagrams
A structural change shows the old shape and the new shape as two labeled block diagrams, before above after. A diff carries changed text only; a reshaped diagram never fits one.

## 8. Write the text around the view

### Put the view where the question lands
The view sits at the point it answers, and one line above it says why this thing is on screen — what it lets the Architect decide, or what changes because of it. The detail inside a view means nothing until that line exists.
Example: `The gate reads the mode from one place, so a wrong answer there refuses every write:` above the tree.
Never: every drawing collected at the end, or a drawing dropped in with no line above it, which lets deep detail arrive with no reason to read it.

### Let prose carry the WHY no single row owns
The view carries what changes, and a row needing its own reason carries it in its annotation. Prose carries the reason that spans the view: why the edge sits there, why the dependency runs that direction, and what is hard.
Never: a paragraph restating what the drawing above it already shows.

### Name the effect on the User, not the mechanism
Say what the User or the system can do differently. The mechanism belongs in the view.
Never: an implementation detail offered as the answer to what a change means.

### Nest the reply the way the Decisions nest
Hierarchy shows what depends on what, and a flat list of thirty items hides it.
Never: a wall of equal-weight bullets, or a heading level that contradicts the dependency.

### List every item when the ask is an inventory
"Every X" means every X. Completeness governs inside the scope named, and brevity governs everything outside it.
Never: a sample, a category summary, or "and others" when the ask was the full list.

### Put the whole deliverable in this one message
The Architect reads the last message only, so everything needed sits inside it.
Never: "as shown above", "see the earlier message", or a pointer to a file the Architect would have to open.

### Let the content set the length
Length follows the facts that earn their place. Cut whole sentences that add nothing, and never compress the words of a sentence worth keeping.
Never: padding a short answer to look thorough, or trimming a Proposal until it stops being reviewable.
