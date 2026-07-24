---
name: architecture
description: TRIGGER when presenting Architectural options, Architecting systems, or discussing tradeoffs. Append to any Prompt.
---

# Architecture

- Architecture Decisions are expensive to reverse.
- The Architect reviews Architecture: files, public APIs, database, ownership, dependency direction, and module boundaries.
- The Agent owns HOW; the Architect owns Architecture.

## 1. Start with WHY and the highest Decision

State the problem, what triggered it, and the highest Architectural Decision before lower-level choices.

### Put the controlling Decision first
Defaults, naming, edge cases, and implementation details wait until the data model, module boundary, public API, and dependency direction are approved.
Never: ask about defaults before the data model, naming before the Architecture, edge cases before the happy path, or implementation details before the approach.

### Separate Architecture, Convention, and implementation
Architecture is module boundaries, public contracts, data ownership, dependency direction, new modules, and schema mutations. Convention follows repo Precedent. Implementation is the Agent's tactical work: method internals, error messages, and control flow.
Never: present an implementation choice as an Architectural Decision.

## 2. Break the Architecture before presenting it

Build toward the Architecture the code should have. Current code is evidence, not a wall around the Decision.

### State the hypothesis concretely
Name the shape you think is right in one sentence, plus the Plan and why. Show real names, public API methods, the call site, or pseudocode where useful.
Never: describe the Architecture only as an abstract category.

### Attack the hypothesis instead of confirming it
Trace the code it touches and find the case it cannot handle, the boundary that does not hold, the capability the User would lose, or the caller it forces you to rewrite. When using Subagents, ask them to break the proposed Architecture and propose fixes.
Never: ask a Subagent to summarize the current code when the Task is to harden a proposed Architecture.

### Fix every weak spot before Review
A weak spot is a defect in the Architecture, not a tradeoff to list and move past. Fold in fixes and attack again until the shape is coherent, elegant, and functional.
Never: unbroken hypothesis, status-quo wall, or noted-not-fixed.

## 3. Show the Architecture before the options

Use /show-architecture to show what exists and what changes.

### Show the call site
The option must show what the caller writes to use it, not only the data model.
Example: show the public method call and payload shape beside the ownership change.
Never: only diagram tables while hiding the public API the caller will use.

### Use /naming for identifiers in Examples
Names are Architecture when they enter files, public APIs, database schema, or reusable vocabulary.
Never: invent identifiers inside an Architectural Example.

## 4. Present only genuine options with /pcc

Every option needs pros, cons, and confidence so the Architect can compare it.

### Keep options in different tradeoff spaces
Each option must solve at least one problem the others do not. State what each option is best for; if two options are best for the same thing, merge them.
Never: two options that are the same idea.

### State the Convention each option creates
Good Architecture eliminates future Decisions. Name what choosing the option forces the project to decide next.
Example: "Choosing this makes the entity own the public API; controllers stop assembling the payload."

### Reject filler options before the Architect sees them
An option must be viable in real Execution. Do not present exits from the direction the Architect is exploring.
Never: "defer", "use an external service" for a simple thing, "code-only" when runtime control is needed, "keep current", "start over", "abandon this direction", or any option you would not recommend.

## 5. Check ownership and coupling

Ask what each system knows, what it does not know, and who owns each concept.

### Keep one Owner per concept
Do not couple unrelated concerns. Same shape does not mean same concern when lifecycles differ.
Example: billing is not tenancy; plans are not feature flags.
Never: make one system change whenever an unrelated system changes.

### Frame cost as maintenance burden against revenue
Compare lines of code and maintenance burden against revenue potential at 1,000 Users: retention, upsells, and reduced churn.
