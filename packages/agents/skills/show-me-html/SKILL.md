---
name: show-me-html
description: Build and publish the page the Architect reviews an Architecture Proposal on — one self-contained page toggling between the start state and the final state across architecture diagrams, annotated file trees, and database changes. TRIGGER when the Architect fires this Command by name. DO NOT TRIGGER to show Architecture inline in a reply (use /show-me) or to review code changes (use /review).
disable-model-invocation: true
---

# Show Me HTML

- The published Artifact URL is the deliverable; the Architect reviews at that URL.
- The source document is the authority for what the review says.
- The Architect reads the page without having read the source document.

## 1. Author the start state and the final state

Read the source document, then resolve every claim it makes against the code with /trace. Each subject — a component, a layer, a file, a table — gets exactly two recorded states.

### Record the state before all changes and the state after all changes
The two states are the tree, the API, and the schema before any change lands, and the same after every change lands. Nothing between them is recorded.
Example: a method added in one Slice and removed in a later one appears in neither state.
Never: an intermediate state, a per-Slice snapshot, or a path the Proposal ruled out.

### Measure both states against HEAD
The start state is what HEAD holds and the final state is what HEAD holds plus every change the source document proposes. Read each start state with `trace read <file> --at HEAD`.
Never: reading the start state from the working tree, where landed edits already show as "before".

### Record the two states without narrating how the change got there
The page states what is and what will be. Nothing on it recounts Slice order, earlier attempts, or what an intermediate revision looked like.
Never: "originally X, then Y, now Z", a changelog, or a Slice-by-Slice history.

### Keep every subject at the file, database, and API layer
Record files, public methods, tables, columns, and endpoints. Internals below that layer are not part of the review.
Never: control flow, private helpers, or line-level edits.

### Show a missing specific as an "unspecified" marker
A path, name, or value the source document does not state renders as an explicit "unspecified" marker in that state. A marker is the correct output, so it never fails Verification.
Never: invent a filename, a method signature, or a column type to complete a state.

## 2. Cover the Architecture, every changed file, and every changed table

Every subject the change touches appears in one of three sections. Coverage is complete or the review is wrong.

### Draw one global architecture diagram and one per section
The global diagram carries the whole change. Each section carries its own diagram of the part it owns. Both hold the start state and the final state.

### Put every changed file in an annotated tree with its touched public methods
Trees follow /show-me. Under each file, list the public methods the change adds, removes, or alters, in both states, so the Architect reviews the public API. The tree also carries the direct context needed to explain the change — a caller, a callee, a sibling implementing the same contract — with the same method list.
Never: a tree that omits a changed file, a tree holding only the changed files, or a changed file listed without its touched methods.

### Give every changed table its structure in both states
List columns, types, keys, and indexes. When the storage format changes, show the stored shape before and after.
Never: naming a table as "updated" without its structure.

### Give each table its own detail card
One card per table, holding that table's name, its columns with types, its keys, its indexes, and its relations. The database section is these cards.
Never: one combined schema table covering several tables, or a table whose detail exists only on hover.

### Show the change instead of describing it
The page carries diagrams, trees, tables, and highlighted code. Code appears as code, bounded to the public signatures and structural excerpts of the touched APIs.
Never: a paragraph explaining what a diagram would have shown, or a method body.

## 3. Wire the toggle dock

One control switches the whole page between the two states.

### Put one global toggle in a static dock
The dock stays fixed on the page. Flipping it swaps every diagram, tree, table, and snippet at once, per component and per layer, in place.
Never: a before section followed by an after section, a per-section toggle, or a linear sequence the Architect scrolls through.

### Color-code added, deleted, and modified so the status reads at a glance
One color carries one status across the whole page, and the status is legible without reading the label.

### Put the depth on hover
Hovering a file reveals its public methods and hovering a table reveals its exact structure, in the state the toggle currently shows. Both states carry the same depth, and every value matches what /trace returned.
Never: hover content authored for one state only, or a signature not read from the code.

## 4. Carry the Decisions

### Give every Decision its options, the pick, and the reason
Each Decision shows the options considered in the /pcc shape, which one was picked, and why.
Never: the picked option alone.

## 5. Design the page

Follow /design.

### Move only on a state change
Only the toggle flip and the hover reveals animate; nothing else moves.

## 6. Publish fresh and verify

### Write a new file and publish a new Artifact every iteration
Each iteration builds the page from the current state of the source document into a new file path, then publishes that file as a self-contained page with every style and script inline.
Never: editing the previous iteration's file, or republishing over its URL.

Verification:
  - Every changed file in the source document appears in a tree, with its touched public methods in both states.
  - Every detail the source document specifies renders exactly; a detail it leaves out renders as an "unspecified" marker.
  - Every changed table appears with its columns in both states.
  - The global diagram and every section diagram both render in both toggle positions.
  - Every Decision in the source document appears with its options, its pick, and its reason.
  - Opening the published URL, flipping the dock toggle, and hovering one file and one table shows the swap and the depth.
