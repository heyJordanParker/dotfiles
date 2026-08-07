# Shaping documents

Use this Reference when Shaping needs Prompt files under `docs/shaping/[feature]/` or researcher Prompts.

## 1. Create the Shaping directory

### One feature gets one directory
All Shaping Prompt files for a feature live in `docs/shaping/[feature]/`. Create the directory when it does not exist.

## 2. Capture the source

### `frame.md` holds the WHY
Write source material, Problem, and Outcome in `frame.md`. Source material is verbatim; Problem and Outcome are distilled.

Template:
  ```markdown
  ---
  shaping: true
  ---

  # [Feature Name] — Frame

  ## Source

  > Original Architect, stakeholder, email, scenario, or raw material.

  ## Problem

  ...

  ## Outcome

  ...
  ```

### Preserve raw material
Quotes, emails, stakeholder material, scenarios, and any raw material that informs the work stay in Source so later Agents can revisit the original Context.

## 3. Write the Shaping Prompt

### `shaping.md` is the ground truth for Shaping
Exploration, R, X, shapes, parts, and fit checks live in `shaping.md`.

Template:
  ```markdown
  ---
  shaping: true
  ---

  # [Feature Name] — Shaping

  ## Energy Level

  ## Requirements

  ## Boundaries

  ## Shapes

  ## Fit Check
  ```

## 4. Write the Modeling artifact through `/modeling`

### `affordances.md` is the Modeling source of truth
`/modeling` writes UI Affordance, Code Affordance, Data Store, Wires Out, and Returns To tables in `affordances.md`.

Template:
  ```markdown
  ---
  modeling: true
  ---

  # [Feature Name] — Modeling
  ```

## 5. Write Slices through `/slicing`

### `slices.md` is the ground truth for Slice definitions
`/slicing` writes Slice definitions with acceptance criteria and Verification. It references R and X from `shaping.md`.

### Slice Plans are self-contained
Each `V1-plan.md`, `V2-plan.md`, and later Slice Plan carries the WHY from `frame.md` and the implementation detail for that Slice.

## 6. Keep Prompt levels aligned

### Changes ripple both ways
When Shaping changes, update Slices. When a Slice Plan reveals a new mechanism, update `slices.md` and `shaping.md` in the same operation.

Template:
  ```text
  frame.md
      ↓
  shaping.md
      ↓
  affordances.md
      ↓
  slices.md
      ↓
  V1-plan.md, V2-plan.md
  ```

## 7. Create research Prompts for unknown mechanics

### Research files are standalone
Create `research-[topic].md` in the feature's Shaping directory. A research Prompt learns mechanics and concrete steps; it does not make the Decision.

Template:
  ```markdown
  # [Component] Research: [Title]

  ## Context

  Why we need this investigation. What problem we are solving.

  ## Goal

  What we are trying to learn or identify.

  ## Questions

  | # | Question |
  |---|----------|
  | X1-Q1 | Specific mechanics question |
  | X1-Q2 | Another mechanics question |

  ## Acceptance

  Research is complete when all questions are answered and we can describe [the understanding we will have].
  ```

### Research questions ask about mechanics
Ask where logic lives, what changes are needed, how the system performs an action, and what boundaries affect an approach.
Never: effort estimates, vague difficulty questions, or yes/no questions that do not reveal mechanics.

### Research acceptance names information
Acceptance describes the understanding you will have after research.
Example: we can describe how Users set their language and where non-English titles appear.
Never: we can decide if we should proceed.
