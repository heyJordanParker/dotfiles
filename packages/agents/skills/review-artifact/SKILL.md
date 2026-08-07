---
name: review-artifact
description: Build and publish the Artifact the Architect reviews an Architecture Proposal in — one self-contained page toggling between the start state and the final state across architecture diagrams, annotated file trees, and database changes. TRIGGER on "make a review artifact", "publish the review", "artifact showing before/after", "render this plan for review", "build the review page". DO NOT TRIGGER to show Architecture inline in a reply (use /show-architecture) or to review code changes (use /review).
---

# Review Artifact

- The published Artifact URL is the deliverable; the Architect reviews at that URL.
- The source document is the authority for what the review says.
- The Architect reads the page without having read the source document.

## 1. Author the start state and the final state

Read the source document, then resolve every claim it makes against the code with /trace. Each subject — a component, a layer, a file, a table — gets exactly two recorded states.

### Record the state before all changes and the state after all changes
The two states are the tree, the API, and the schema before any change lands, and the same after every change lands. Nothing between them is recorded.
Example: a method added in one Slice and removed in a later one appears in neither state.
Never: an intermediate state, a per-Slice snapshot, or a path the Proposal ruled out.

### Keep every subject at the file, database, and API layer
Record files, public methods, tables, columns, and endpoints. Internals below that layer are not part of the review.
Never: control flow, private helpers, or line-level edits.

### Show a missing specific as a gap
A path, name, or value the source document does not state stays visible as an unfilled gap.
Never: invent a filename, a method signature, or a column type to complete a state.

## 2. Cover the Architecture, every changed file, and every changed table

Every subject the change touches appears in one of three sections. Coverage is complete or the review is wrong.

### Draw one global architecture diagram and one per section
The global diagram carries the whole change. Each section carries its own diagram of the part it owns. Both hold the start state and the final state.

### Put every changed file in an annotated tree with its public methods
Trees follow /show-architecture. Under each file, list every public method of that file in both states, so the Architect reviews the public API.
Never: a tree that omits a changed file, or a file listed without its methods.

### Give every changed table its structure in both states
List columns, types, keys, and indexes. When the storage format changes, show the stored shape before and after.
Never: naming a table as "updated" without its structure.

### Show the change instead of describing it
The page carries diagrams, trees, tables, and highlighted code. Code appears as code.
Never: a paragraph explaining what a diagram would have shown.

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

Follow /design in minimal mode.

### Build hierarchy from spacing, size, and weight
Separate sections with space and type scale.
Never: divider lines, one-sided accent borders, gaudy gradients, or purposeless purple-blue-pink palettes.

### Move only on a state change
The toggle flip and hover reveals animate. Nothing else does.

## 6. Publish fresh and verify

### Write a new file and publish a new Artifact every iteration
Each iteration builds the page from the current state of the source document into a new file path and publishes that. Follow /artifact-design for the publish itself.
Never: editing the previous iteration's file, or republishing over its URL.

Verification:
  - Every changed file in the source document appears in a tree, with its public methods in both states.
  - Every changed table appears with its columns in both states.
  - The global diagram and every section diagram both render in both toggle positions.
  - Every Decision in the source document appears with its options, its pick, and its reason.
  - Opening the published URL, flipping the dock toggle, and hovering one file and one table shows the swap and the depth.
