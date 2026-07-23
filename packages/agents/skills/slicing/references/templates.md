# Slicing templates

Use this Reference when writing `slices.md`, `V*-plan.md`, Plan-mode symlinks, per-Slice Affordance tables, or optional Mermaid Slice-scope styling.

## 1. Write `slices.md`

### `slices.md` summarizes the whole Plan
Write `docs/shaping/[feature]/slices.md` with the Plan contract, WHY from `frame.md`, scope from Shaping, boundaries, and a Slice summary.

Template:
  ```markdown
  ---
  shaping: true
  ---

  # [Feature Name] — Plan

  > Plan contract: This Plan is an immutable contract. Architectural deviations require explicit approval.

  WHY: [One paragraph — business motivation from frame.md]

  Scope: [Natural-language scope from Shaping]

  Boundaries:
  - [From Shaping plus any discovered during Slicing]

  ## Slice Summary

  | # | Slice | Parts | Requirements | Affordances | Demo |
  |---|-------|-------|--------------|-------------|------|
  | V1 | ... | F1, F4 | R0, R3 | U2-U5, N3-N8 | "..." |

  ## V1: [Slice Name]

  Parts: F1, F4
  Requirements: R0, R3
  Demo: "[Concrete demo statement]"

  Files:
  - Create: exact/path/to/file.ext
  - Modify: exact/path/to/existing.ext

  Reuses:
  - Existing module/component from exact/path

  Precedent:
  - Each Architectural pick builds on exact/path/to/precedent, or searched exact locations and no Precedent exists.

  Regression scope:
  - Existing behavior in exact/path — verify with exact command or manual check.

  Acceptance Criteria:
  - [ ] Functional: ...
  - [ ] Regression: ...
  - [ ] Dependency audit: ...
  - [ ] Boundary: ...

  Verification:
  - [ ] How each criterion gets verified — which Agent, which command, which browser check.
  ```

## 2. Write each `V*-plan.md`

### Each Slice Plan is self-contained
Write one file per Slice: `docs/shaping/[feature]/V*-plan.md`. Include WHY, scoped requirements, modeled changes, Affordances, Tasks, acceptance criteria, and changed file tree.

Template:
  ```markdown
  # [Feature] — V[N]: [Slice Name]

  > Plan contract: This is an immutable contract. Architectural deviations require explicit approval. Tactical code-level adjustments are fine.

  > For Claude: Use the /subagents Skill to dispatch implementation.

  WHY: [What User problem this Slice solves]

  ## Requirements

  | ID | Requirement | Status |
  |----|-------------|--------|
  | R0 | [only the Rs this Slice satisfies] | Must-have |

  ## Modeled Changes

  [Database tables + columns, annotated NEW/REMOVED/RENAMED]

  accounts
  ├── id: int
  ├── email: string
  ├── password: ?string                  <- NEW nullable
  ├── setup_token: ?string              <- NEW
  └── tenant_id: int                     <- FK → tenants

  ## Affordances

  | # | Component | Affordance | Control | Wires Out | Returns To |
  |---|-----------|------------|---------|-----------|------------|
  | U1 | ... | ... | ... | ... | ... |

  ## Task 1: [Component Name] (fulfills R0, R1)

  Files:
  - Create: exact/path/to/file.ext
  - Modify: exact/path/to/existing.ext

  Precedent:
  - Each Architectural choice builds on exact/path/to/precedent, or the search proving none exists.

  Step 1: [Action — exact code or specific Task step]

  Step 2: [Verification — command, API call, browser check, command output, or observable result]

  ## Task N: Verification (fulfills all Rs)

  Dispatch independent testing Subagents via /subagents.
  - Trace every code path touched.
  - Validate real User scenarios end to end.
  - Browser-test via tester Agent and /agent-browser when User Interface is involved.
  - Run every test category fitting scope.
  - Fix all issues that do not change the Plan's Architecture.
  - Present results to the Architect for manual Verification and feedback; do not commit without approval.

  ## Acceptance Criteria

  - [ ] Functional: ...
  - [ ] Regression: ...
  - [ ] Dependency audit: ...
  - [ ] Boundary: ...
  - [ ] Verification: feature works end to end from a User perspective.

  ## Changes

  app/
  ├── Models/Account.php*              <- +password, +setup_token
  ├── Controllers/AuthController.php*  <- +setPassword(), +sendResetLink()
  ├── Services/EmailService.php*       <- NEW extracted from tenant
  admin/
  ├── pages/SetPassword.tsx*           <- NEW
  ├── pages/Login.tsx*                 <- +forgot password toggle
  ```

## 3. Write Tasks as do then verify

### Each Task step is small and observable
Use exact file paths and complete code where the Plan needs to constrain Architecture. Each Task step includes concrete Verification.
Never: a Task with only prose and no observable result.

## 4. Create Plan-mode symlinks

### Symlink every Slice Plan
Create a symlink so Claude Code Plan mode can reference each Plan.

Template:
  ```bash
  ln -s docs/shaping/[feature]/V1-plan.md ~/.claude/plans/[descriptive-name].md
  ```

### Symlinks are transparent
Claude Code reads and writes through the symlink to the repository Plan file.

## 5. Extract per-Slice Affordance tables

### Each Slice gets only the Affordances being added
Copy the Affordances assigned to the Slice into a focused table. Wires to later Slices stay as stubs until that later Slice is built.

Example:
  ```markdown
  ## V2: Search Works

  | # | Component | Affordance | Control | Wires Out | Returns To |
  |---|-----------|------------|---------|-----------|------------|
  | U1 | search-detail | search input | type | → N1 | — |
  | N1 | search-detail | activeQuery.next() | call | → N2 | — |
  | N2 | search-detail | activeQuery subscription | observe | → N3 | — |
  ```

## 6. Add optional Mermaid Slice scope

### Slice diagrams distinguish scope
Use the complete Modeling output and style Affordances by Slice state.

Template:
  ```mermaid
  classDef thisSlice fill:#90EE90
  classDef built fill:#d3d3d3
  classDef future fill:none,stroke-dasharray:3 3
  ```

### The three states are fixed
This Slice is bright. Already built is solid grey. Future is transparent with a dashed border.
