---
name: show-architecture
description: Shows Architecture inline with annotated file trees, runtime relationship diagrams, and narrow markers. TRIGGER when exploring or explaining code, changing Architecture, or when the Architect asks to visualize Architecture.
---

# Show Architecture

- Show Architecture inline in the reply.
- Never write files.
- Use the narrowest view that explains the Architecture.

## 1. Choose the view

### Use a file tree for location and change shape
A file tree shows where code lives, which files change, and how nearby files relate.
Example: use a file tree when explaining what exists and what changes.

### Use a relationship diagram for runtime behavior
A relationship diagram shows movement, ownership, call order, or end-to-end behavior.
Example: use a relationship diagram when the question is how data moves through the system.

### Use markers only when the symbol carries meaning
Markers are for checklist state, two-way relationships, or formulas. They are not general shorthand.
Never: scatter symbols through prose as decoration.

## 2. Draw file trees when location matters

### Use box-drawing characters and short annotations
Use `├──`, `└──`, and `│` for structure. Use `<-` annotations with three to five words, adapted to the purpose.
Template:
  ```
  directory/
  ├── file.ts*             <- annotation (3-5 words)
  ├── subdirectory/
  │   ├── nested.ts*       <- changed file marked with *
  │   └── related.ts       <- context file (no *)
  └── context.ts
  ```

### Mark changed files with `*`
A changed file gets `*` suffix. An unchanged file gets a plain role annotation and no status prefix. Skip irrelevant files entirely.
Never: `KEEP:`, `REMOVE:`, `PRESERVE:`, `* new`, or `existing,` prefixes.

### Match annotations to the purpose
Overview annotations name responsibility. Feature annotations name data movement. Debugging annotations name dependency or failure location.
Example: `engine.ts* <- orchestrates subsystems`; `validate.ts* <- checks credentials`; `UserRepo.ts* <- fails here`.
Never: annotations that repeat the filename or a tree with no annotations.

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
                     └────────┬─────────┘
                              │ charge
                              ▼
                     ┌──────────────────┐
                     │ Gateway plugin   │
                     └──────────────────┘
  ```

### Label every arrow with the relationship
Arrow labels name the behavior: `read`, `write`, `charge`, `emit`, or `call`. Run top to bottom from entry to terminal effect.
Never: unlabeled arrows.

### Show only the path being explained
The diagram is not an inventory. Include only boxes on the path that answers the question.
Never: every related component in the system.

## 4. Use narrow markers

### Use `○` and `●` for checklist state
Use open and done markers only for checklist state.
Example:
  ```
  release readiness
  ● schema migration applied
  ● checkout endpoint live
  ○ frontend wired to the new endpoint
  ○ end-to-end test passing
  ```

### Use `↔` for a two-way relationship
Use the symbol only when both sides affect each other.
Example: `CheckoutView ↔ CartService`.

### Use equation symbols for relations and formulas
Use `=`, `≠`, `≈`, `≤`, `≥`, `±`, and `×` for compact relation or formula lines.
Example: `order.line_items ≥ 1`; `tax = subtotal × 0.2`.
