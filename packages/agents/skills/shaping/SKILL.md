---
name: shaping
description: Collaborative Shaping with the Architect: capture requirements, boundaries, shapes, fit checks, research unknown mechanics, and transition to Modeling then Slicing. TRIGGER when the Architect asks to shape, scope, compare approaches, or turn a rough idea into a modeled change.
---

# Shaping

- Shaping defines the problem and compares shapes before Execution.
- Shaping is the first part of `Shaping → Modeling → Slicing`.
- Requirements, boundaries, and shapes can change each other in any order; every checkpoint shows the current state of all three.
- `CURRENT` is the reserved shape name for the existing system.

## 1. Start from the Architect's entry point

### Offer the two valid starts
Start from requirements when the Architect describes pain and wants shapes to emerge. Start from a shape when the Architect sketches a path and requirements need to be extracted.
Example: "from R" means describe the pain first. "from S" means sketch a shape first.

### Match the energy level
Broad refactor energy earns broader Architecture options. Careful adjustment energy stays focused. When cutting scope, the remaining shape must still beat what Users have now.

## 2. Maintain requirements as R

### R states what is needed
Use `R0, R1...` for the problem space. R is negotiated with the Architect, not auto-filled by the Agent. R says what must be true, not which shape satisfies it.

### Keep top-level R readable
Use no more than nine top-level Rs. Group overflow as `R3.1`, `R3.2`.

Template:
  ```markdown
  | R | Requirement | Status |
  |---|-------------|--------|
  | R0 | Core Goal | Core Goal |
  | R1 | User can return to the same search state | Must-have |
  | R2 | Search survives page refresh | Nice-to-have |
  ```

## 3. Maintain boundaries as X

### X is a firm no
Use `X0, X1...` for what this work does not cover. Boundaries are specific and testable.
Example: no public API changes; no migrations.
Never: keep it simple; do not break anything.

IF an R conflicts with an X:
### Surface the conflict instead of dropping either side
Stop and renegotiate. Do not silently remove the R or boundary.

## 4. Maintain shapes as S

### Shapes are mutually exclusive approaches
Use `A`, `B`, and `C` for approaches where the Architect picks one. Give each shape a short title that captures the approach.
Example: `C: Two data sources with hybrid pagination`.

### Components combine within a shape
Use `C1`, `C2` for parts that compose inside a selected shape. Use `C3-A`, `C3-B` for alternatives inside a component.
Example: `Shape E = C1 + C2 + C3-A`.

### Preserve notation as an audit trail
Do not renumber old parts just to make the table prettier. The notation shows what changed across iterations.

## 5. Write shape parts as mechanisms

### Parts describe what changes
A part names the thing built or changed. It does not restate intent, boundary, or R.
Example: `B1: Signing handler — includes WaiverSignatures table + handler`.
Never: `B4: Data model`.

### Parts are vertical
Co-locate the data model with the feature it serves. Extract shared logic into a standalone part that other parts reference.
Example: `B1: Signing handler`, then `B2 → calls B1` and `B3 → calls B1`.

### Add hierarchy only when flat stops working
Start with `E1`, `E2`. Use `E1.1` only when there are too many parts or the final file layout must be shown.

### Name Precedent on selected parts
Each selected shape part names the existing file or system it builds on, with its path, or the research proving none exists.

## 6. Run the fit check

### The fit check decides which shape to pursue
Use one table: requirements as rows, shapes as columns.

Template:
  ```markdown
  | R | Requirement | A | B | C |
  |---|-------------|:-:|:-:|:-:|
  | R0 | Core Goal text | ✅ | ✅ | ❌ |
  | R1 | Full requirement text | ❌ | ✅ | ✅ |
  ```

### Shape columns are binary
Use ✅ or ❌ only in shape columns. Put short explanations in a minimal Notes section below the table.

### Unknown mechanics fail the fit check
A flagged unknown means the WHAT is described but the HOW is not. The shape is ❌ until the unknown is researched and cleared by a Decision.
Never: mark ⚠️ as passing.

IF a shape passes every R but still feels wrong:
### Name the missing R and rerun the fit check
The discomfort means the table is missing a requirement.

## 7. Research unknown mechanics

IF mechanics or feasibility are uncertain:
### Dispatch one researcher Subagent per unknown
Research learns how the existing system works and what it would take to build a part before proposing it. Dispatch multiple researcher Subagents in parallel when multiple unknowns exist.

### Research acceptance names understanding, not a Decision
Acceptance says what the Agent will be able to describe after research. The Decision clears the flag afterward; the research does not.
Example: we can describe how Users set their language.
Never: whether this is a blocker.

## 8. Surface every checkpoint

### Show the full current state
After any iteration that changes R, X, shapes, fit checks, or Modeling, present requirements and statuses, boundaries, outstanding Decisions, and what changed.

### Mark changed cells
Use 🟡 on every table cell that changed since the previous checkpoint. Show every R, every shape part including sub-parts, and every alternative.

## 9. Transition to Modeling and Slicing

### Keep Prompt levels consistent
Shaping Prompt holds R, X, shapes, and fit checks. Slices Prompt holds Slice definitions. Slice Plans hold implementation detail. A change at any level ripples both ways; update every affected level in the same operation.

IF a shape is selected and the fit check passed:
### Invoke `/modeling`
Model the selected shape into Affordances before Slicing.

IF Modeling is complete:
### Invoke `/slicing`
Break the modeled change into vertical Slices. Every Slice ends in demo-able functionality.

## References

- Writing Shaping Prompt files, source capture, file layout, frontmatter, and research Prompts → [documents.md](references/documents.md)
- Running a high-level Addressed and Answered fit check for chunked requirements and mostly-unknown shapes → [macro-fit-check.md](references/macro-fit-check.md)
