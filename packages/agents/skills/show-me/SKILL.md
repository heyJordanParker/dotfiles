---
name: show-me
description: Draw every code change in the reply as a diff, an annotated file tree, or a class diagram, and keep ideas in prose. TRIGGER on every reply that explains code, Architecture, a change, a name, or a finding, and whenever the Architect says "show me". DO NOT TRIGGER to score a set of options; that is /rank.
---

# Show Me

- Prose about code is slow to read and slower to understand, and it costs the Architect a wrong reading of the change or an Architecture he would have improved.
- Every drawing lives in the reply. This Skill writes no files.

## 1. Draw every code change, and keep ideas in prose

The Architect needs to validate the Architecture and can substantially improve it, and he does not read the code. An idea belongs in a sentence. A change to code belongs on screen, drawn, with enough of what surrounds it for him to judge whether it is right and whether it fits what is already there. You decide how much that is.

### Show a public API change
Agents lose the big picture and add a bespoke method per caller, so a service ends up carrying dozens of near-duplicates that nobody can tell apart. Putting the API change in front of the Architect stops that before it becomes debt.
Example:
  ```diff
  // app/Tenant/Community/SpaceService.php
  + #[Action(Mode::Destroy, disabled: 'hasProducts == true')]   // new; refuses while a Product grants it
    public function delete(
        Space $space,                   // unchanged
    ): void
  - public function detachProducts(Space $space): void          // removed; product_spaces cascades
    public function create(string $name, ?Space $parent): Space // unchanged
  ```
Note: show every public API method on every changed file. Unchanged methods are what make a congruency review possible.

### Show a public API change that spans files
Spread over files, a change reads as a list of edits, and which module ends up owning what disappears. An annotated file tree puts the whole surface in one place, so a responsibility landing in the wrong module is visible.
Example:
  ```diff
    app/Tenant/Store/
    ├── DeliveryService.php                                             <- one public method now
  + │   ├── update(OrderOffer $bought): void                            <- access follows the payer
  - │   ├── deliverOrder(int $orderId): void                            <- replaced by update
  - │   ├── deliverItem(SubscriptionItem $item, Contact $buyer): void   <- replaced by update
  - │   └── revoke(Contact $buyer, GrantSource $source, int $id): void  <- replaced by update
    └── Entities/Subscription.php                                       <- belongs to one Offer now
  +     ├── offer(): BelongsTo                                          <- the Offer it was bought on
  -     ├── items(): HasMany                                            <- each item is its own Subscription now
  -     ├── activeItems(): HasMany                                      <- same
  -     ├── currentAmount(): int                                        <- read off the Offer snapshot
        └── status(): SubscriptionStatus                                <- unchanged
  ```
Note: show every public API method on every file the change reaches, changed or not.

### Show a critical logic change
The Architect follows the logic to judge whether the code is complete, and the changed lines alone do not carry the flow.
Example:
  ```diff
  // app/Tenant/Store/DeliveryService.php  ::  update()
    public function update(OrderOffer $bought): void
    {
        // resolves the payer: the Subscription, or the Order when there is none
  -     if ($bought->delivery !== null) {
  -         return;                                  // the uuid column answered this
  -     }
  +     if ($this->automations->runFor($payer, $bought) !== null) {
  +         return;                                  // the owner and cause answer it now
  +     }
        // grants every Space and Course in the snapshot, then starts the run
    }
  ```
Note: summarize the rest of the method in comments, and show the part under review in full.

### Show a database change
A migration does not show the storage shape, and the shape is what the Architect reviews.
Example:
  ```diff
    funnel_steps
  +   offer_selection string NOT NULL DEFAULT 'all'   <- how the step sells its offers
  -   settings                                        <- the untyped bag it replaces
      funnel_id bigint
      slug string
      name string
    step_offers
  +   is_default boolean                              <- ticked when the page loads
  ~   conditions jsonb → when text
      step_id bigint
      offer_id bigint
      sort_order integer
  ```
Note: show every column in the table, changed or not.

### Show Architecture
The Architect decides the Architecture. Prose about which class calls which leaves him agreeing to a shape he never saw, and a shape he never saw is one he cannot improve.
Example:
  ```
  ┌───────────────────────┐
  │ AccessService         │
  │ check() / admission() │
  └──────┬────────────────┘
         │ calls includes()
         ▼
  ┌───────┐  implements   ┌──────────┐
  │ Owner │ ────────────> │ Audience │
  └───────┘               └──────────┘
  ```
Note: draw the classes the change leaves alone as well as the ones it touches.

### Show a Prompt change
Prompts are how the Architect controls Agent behaviour over time, and Agents write Prompts badly, so he reviews every change in full.
Example:
  ```diff
    ### Name every reference inline
  + The Architect resolves every name from the reply itself. He has no file open.
    Never: "as above", "from earlier", "see point 3", or "the slice above".
  + Never: "L45", "lines 76-77", "section 2", "the third heading".
  ```

### Never use tables
Tables take too long to read and understand. An AI's tables take even longer.

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
Overview annotations name responsibility. Feature annotations name data movement. Debugging annotations name dependency or failure location. When the row's name already carries that, the annotation carries the reason instead: why the file exists, why it changed, why it stays.
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

## 3. Draw diagrams when runtime behavior matters

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
A runtime diagram follows one path, so it holds the boxes that path reaches. A diagram of the Architecture holds every class in the module, touched or not.

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
Never: a state diagram for a lifecycle with one path and no branch.

## 4. Show code lines when the Decision lives in the lines

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

## 5. Diff every change

### Diff the structure the change lives in
The `+` and `-` lines work on any structure, not only file text. Diff a file tree, a table's columns, or a step list the same way.
Example:
  ```diff
    1. Pick the shape from the situation
       Show a method or API change as its signatures
  +    Show a change across several files as an annotated file tree
  - 4. Use narrow markers
  + 4. Show code instead of prose about code
  ```

### Change only the lines that changed
Keep every untouched line byte-identical, so only the changed lines differ.
Never: reformatting, re-wrapping, or re-ordering the lines around the edit.

### Show one diff per subject
For text, a tree, or a step list, one diff carries both states. Two blocks side by side make the reader hold four columns at once.
Never: a before block followed by an after block for a change one diff can carry.

### Draw architecture before and after as two diagrams
A structural change shows the old shape and the new shape as two labeled block diagrams, before above after. A diff carries changed text only. A reshaped diagram never fits one.

## 6. Write the text around the drawing

### Put the drawing where the question lands
The drawing sits at the point it answers, and one line above it says why this thing is on screen: what it lets the Architect decide, or what changes because of it. The detail inside a drawing means nothing until that line exists.
Example: `The gate reads the mode from one place, so a wrong answer there refuses every write:` above the tree.
Never: every drawing collected at the end, or a drawing dropped in with no line above it, which lets deep detail arrive with no reason to read it.

### Let prose carry the WHY no single row owns
The drawing carries what changes, and a row needing its own reason carries it in its annotation. Prose carries the reason that spans the drawing: why the edge sits there, why the dependency runs that direction, and what is hard.
Never: a paragraph restating what the drawing above it already shows.

### Name the effect on the User, not the mechanism
Say what the User or the system can do differently. The mechanism belongs in the drawing.
Never: an implementation detail offered as the answer to what a change means.

### Let the content set the length
Length follows the facts that earn their place. Cut whole sentences that add nothing, and never compress the words of a sentence worth keeping.
Never: padding a short answer to look thorough, or trimming a Proposal until it stops being reviewable.
