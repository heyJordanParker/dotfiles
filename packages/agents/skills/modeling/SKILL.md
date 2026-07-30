---
name: modeling
description: Transform Shaping output into a concrete Modeling artifact — database schema, User experience, and Architecture. TRIGGER between Shaping and Slicing, or when the Architect asks to model an existing system before Execution.
---

# Modeling

- Modeling turns Shaping output, an existing system, or both into `docs/shaping/[feature]/affordances.md`.
- `affordances.md` is the source of truth and carries frontmatter `modeling: true`.
- The Agent builds Affordance tables first; the Architect sees Database Schema, User Experience, and Architecture.
- One Modeling artifact tells the full story across frontend and backend. Label which Affordances belong to the existing system and which belong to the new system.

## 1. Classify the Modeling job

IF mapping an existing system:
### Trace one real User path before listing Affordances
Pick one path, such as land on `/search`, type, scroll, and click a result. List the Places it crosses, then trace from the entry point through every component touched.

IF building from shaped parts:
### Translate every shaped part into concrete Affordances
Write each part from Shaping, then name the UI Affordances, Code Affordances, and Data Stores it needs. Every UI Affordance needs a Code Affordance or Data Store feeding it.

### Model existing and new systems together
Document existing Affordances and new Affordances in the same tables when the change connects them. A new Affordance with no wire to the existing system is either incomplete or outside this Modeling artifact.

## 2. Build the Affordance tables

Write Places, UI Affordances, Code Affordances, and Data Stores before writing the Architect-facing sections.

Template:
  ```markdown
  ---
  modeling: true
  ---

  | # | Place | Component | Affordance | Control | Wires Out | Returns To |
  |---|-------|-----------|------------|---------|-----------|------------|
  | P1 | — | Search page | bounded interaction area | enter | — | — |
  | U1 | P1 | Search page | search input | type | → N1 | — |
  | N1 | P1 | Search page | performSearch() | call | → N2 | → U2 |
  | S1 | P1 | Search page | results | write/read | — | → U2 |
  ```

### A Place is perceptual, not technical
A Place is a bounded interaction area. The test is whether the User can interact with what is behind it. No means a new Place; yes means local state inside the same Place.
Example: modal, whole-screen edit mode, confirmation popover, and backend edge are Places.
Never: URL, component, dropdown, tooltip, or checkbox-revealed fields as Places by default.

### Navigation wires to the Place itself
Wires Out tracks control movement. Navigation points to `P2`, not to a UI Affordance inside `P2`.

### Returns To tracks data movement
Returns To names where an Affordance output goes: a display, a caller receiving a return, or a reader of a Data Store.

### Containment is not wiring
The Place column says where an Affordance lives. Wires Out and Returns To say what moves.

## 3. Fill the tables from code or shaped parts

### Never work from memory
Read the code or the shaped parts. When tracing backwards, scan Wires Out for all Affordances wiring to the target instead of following the path you remember.

### Existing Affordance names must be real
Name the actual method, event, store, or framework mechanism. Never write a generic abstraction when the code has a name.
Example: `userRepo.save()`.
Never: `DATABASE`.

### Mechanisms are not Affordances
A visual wrapper, internal transform, or navigation mechanism is the how, not the Affordance. Wire to the real destination.
Example: `N8 → P3`.
Never: `N8 → N22(modalService.open) → P3`.

### Side effects need Data Stores
A Code Affordance that changes external state wires to a Data Store representing that state.
Example: `updateUrl() → S15(Browser URL)`.

### Data Stores live where they are consumed
Put a Data Store in the Place whose behavior reads it. Lift it only when multiple Places read it.

## 4. Present Database Schema

Use /show-architecture for the tree of models and migrations touched. On top of it here: indent fields under their model, `<- + method(): type` for a new method, `<- existing` for Context.

Template:
  ```text
  app/Models
  └── Order.php *
      ├── status: string <- existing
      └── <- + markPaid(): void
  ```

## 5. Present User Experience

Group pages and components by Critical Path. Each root is tagged `(NEW page)` or `(EXISTING page, MODIFIED)`. Children are Affordances. `submit →` and `click →` show Wires Out inline; `└── success →` shows outcomes; `(REUSED)` marks shared components.

Template:
  ```text
  Checkout (EXISTING page, MODIFIED)
  ├── email input (U1) → N1
  ├── submit button (U2) → N2
  └── success → receipt email (U9)
  ```

## 6. Present Architecture

Open with `## Big Changes`, one to three sentences naming the Architectural shifts. Then write one `## Component (NEW/MODIFIED)` section per component with public methods, signatures, wiring, `Builds on:`, `Uses:`, and `Registered:` when dependency injection applies.

### Stay at the interface level
Architecture covers files, public APIs, and database. Internal transforms stay out unless they are the public contract under review.

### Name Precedent for every component
`Builds on:` names the existing component or full path this follows. If no Precedent exists, show the research that proves it.

## 7. Close the Modeling artifact

Name which Rs from Shaping the Modeling artifact addresses, which stay open, the Decisions the Modeling revealed, and the Architecture edges confirmed unviolated.

### Verify the Affordance count
Count Affordances in the tables, count Affordances in the presentation, and assert equality. Every Affordance appears in exactly one Architect-facing section.

## 8. Continue to Slicing

Invoke `/slicing` after Modeling is complete so the Modeling artifact becomes vertical implementation Slices with acceptance criteria and Verification.

## References

- Classification does not fit Place, UI Affordance, Code Affordance, Data Store, Wires Out, or Returns To → [catalog.md](references/catalog.md)
- Existing system or shaped parts do not translate cleanly into the three Architect-facing sections → [examples.md](references/examples.md)
- The Affordance tables need an optional Mermaid view that preserves Wires Out and Returns To → [mermaid.md](references/mermaid.md)
