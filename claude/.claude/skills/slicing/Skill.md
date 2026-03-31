---
name: slicing
description: Takes a shaped and modeled feature and produces vertical implementation slices with acceptance criteria and validation requirements. Each slice ends in demo-able functionality. The plan becomes an immutable contract.
---

# Slicing

Breaks a shaped and modeled feature into vertical implementation slices. Explores the codebase per slice, generates acceptance criteria and validation requirements, and outputs self-contained plan files.

## Trigger

- Feature is shaped and modeled, ready for slicing
- `/modeling` phase transition invokes this skill

## Input

`~/.claude/shaping/[feature]/shaping.md` with selected shape + `affordances.md`

## Output

- `~/.claude/shaping/[feature]/slices.md` — slice definitions with acceptance criteria
- `~/.claude/shaping/[feature]/V*-plan.md` — individual slice implementation plans
- Symlinks into `~/.claude/plans/` for Claude Code plan mode

---

## Process

### 1. Read the Model

- Read `shaping.md` — extract: WHY, requirements (R), boundaries (X), selected shape parts, scope. If a `frame.md` exists, read it for the original problem statement
- Read `affordances.md` — extract: UI/Code affordances, data stores, wiring
- Read existing code referenced in the model — understand what exists before proposing changes
- Identify which parts create new code vs modify existing code
- If any ⚠️ flags remain on parts, work with the user to resolve them before proceeding

### 2. Break Into Vertical Slices

A vertical slice is a group of UI and Code affordances that does something demo-able. It cuts through all layers (UI, logic, data) to deliver a working increment. The opposite is a horizontal slice — doing work on one layer (e.g., "set up all the data models") that isn't clickable.

**Every slice must have visible output that can be demoed:**
- Has an entry point (UI interaction or trigger)
- Has an observable output (UI renders, effect occurs)
- Shows meaningful progress toward the requirements

**Procedure:**

**Step 1: Identify the minimal demo-able increment**

Ask: "What's the smallest subset that demonstrates the core mechanism working?" Usually: core data fetch + basic rendering. No search, no pagination, no state persistence yet. This becomes V1.

**Step 2: Layer additional capabilities**

Each slice demonstrates a mechanism working. Max 9 slices — if you need more, combine related mechanisms.

**Step 3: Assign affordances to slices**

Go through every affordance and assign it to the slice where it's first needed:

| Slice | Mechanism | Affordances |
|-------|-----------|-------------|
| V1 | Core display | U2, U3, N3, N4, N5 |
| V2 | Search | U1, N1, N2 |

Some affordances may have Wires Out to later slices — that's fine. They're implemented in their assigned slice; the wires are stubs until the later slice is built.

**Step 4: Create per-slice affordance tables**

For each slice, extract just the affordances being added into a focused table:

**V2: Search Works**

| # | Component | Affordance | Control | Wires Out | Returns To |
|---|-----------|------------|---------|-----------|------------|
| U1 | search-detail | search input | type | → N1 | — |
| N1 | search-detail | `activeQuery.next()` | call | → N2 | — |
| N2 | search-detail | `activeQuery` subscription | observe | → N3 | — |

**Step 5: Write demo statement per slice**

Each slice needs a concrete demo: "Type 'dharma', results filter live." Something you can show that demonstrates progress.

**Slice sizing:**
- Too small: 1-2 UI affordances, no meaningful demo → merge
- Too big: 15+ affordances or multiple unrelated journeys → split
- Right size: a coherent journey with a clear demo

For each slice, also:
- List which shape parts the slice implements (using shaping notation — the shape's letter + number, e.g., F1, F2, E1)
- List which requirements (R0, R1...) the slice satisfies

### 3. Explore the Codebase Per Slice

For each slice:
- Read every file that will be modified
- Identify existing patterns to reuse
- Identify existing modules/components that do similar things
- Identify what existing behavior could regress
- Exact file paths for creates/modifies

### 4. Generate Acceptance Criteria

For each slice, generate as sane defaults. Present to user for confirmation.

**4 mandatory categories:**

**Functional** — demo statement works end-to-end, each mechanism produces correct output, edge cases handled (empty states, error states, boundary values)

**Regression** — "Existing [behavior] in [file] still works after changes." Name the behavior, the file, how to verify. If tests exist: "Existing test suite passes." If no tests: "Manual verification of [specific behavior]"

**Dependency audit** — "Verified [module] exists and exposes [method] before using." "No duplicate implementation — checked [locations] for existing [capability]"

**Boundary** — "Does not modify [out-of-scope area]." "Does not introduce [pattern/dependency] outside plan boundaries"

### 5. Generate Validation Requirements

For each slice, specify HOW validation happens. Use the /subagents skill to dispatch validation agents.

**Prompting:** Tell agents WHAT and WHY, never HOW. Scope to the reasoning unit — give each agent the full slice context, not individual files. Avoid bias — don't highlight specific areas of concern.

- Dispatch independent testing agents — never self-validate (the implementing agent is biased)
- Agent prompts include WHY (what user problem this solves), may include WHAT (what changed), never HOW (implementation details)
- Trace every code path touched to verify correctness
- Validate code serves real user scenarios end-to-end
- Browser testing via tester agent + `/agent-browser` when UI is involved
- Exhaust every test category fitting scope (unit, integration, user flows, edge cases)
- Autonomously fix all issues that don't significantly change the plan's architecture
- Final step: present results to user for manual verification and feedback (no commits without approval)

### Visualizing Slices in Mermaid

Optionally, show the complete model in slice diagrams with styling to distinguish scope:

| Category | Style | Description |
|----------|-------|-------------|
| **This slice** | Bright color | Affordances being added |
| **Already built** | Solid grey | Previous slices |
| **Future** | Transparent, dashed border | Not yet built |

Use `classDef thisSlice fill:#90EE90`, `classDef built fill:#d3d3d3`, `classDef future fill:none,stroke-dasharray:3 3`.

### 6. Write slices.md

Output file: `~/.claude/shaping/[feature]/slices.md`

```markdown
---
shaping: true
---

# [Feature Name] — Plan

> **Plan contract:** This plan is an immutable contract. Architectural deviations require explicit approval.

**WHY:** [One paragraph — business motivation from frame]

**Scope:** [Natural-language scope from shaping doc]

**Boundaries:**
- [From shaping doc + any discovered during planning]

## Slice Summary

| # | Slice | Parts | Requirements | Affordances | Demo |
|---|-------|-------|-------------|-------------|------|
| V1 | ... | F1, F4 | R0, R3 | U2-U5, N3-N8 | "..." |

## V1: [Slice Name]

**Parts:** F1, F4
**Requirements:** R0, R3
**Demo:** "[Concrete demo statement]"

**Files:**
- Create: `exact/path/to/file.ext`
- Modify: `exact/path/to/existing.ext`

**Reuses:**
- [Existing module/component] from [location]

**Regression scope:**
- [Existing behavior] in [file] — verify [how]

**Acceptance Criteria:**
- [ ] [Functional criterion]
- [ ] [Regression criterion]
- [ ] [Dependency audit criterion]
- [ ] [Boundary criterion]

**Validation:**
- [ ] [How each criterion gets validated — which agent, which test, which browser check]
```

### 7. Write V*-plan.md Files

For each slice, write an individual plan file: `~/.claude/shaping/[feature]/V*-plan.md`

```markdown
# [Feature] — V[N]: [Slice Name]

> **Plan contract:** This is an immutable contract. Architectural deviations require explicit approval. Tactical code-level adjustments are fine.

> **For Claude:** Use the /team skill to dispatch implementation.

**WHY:** [What user problem this slice solves]

## Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| R0 | [only the R's this slice satisfies] | Must-have |

## Model Changes

[Database tables + columns, annotated NEW/REMOVED/RENAMED]

accounts
├── id: int
├── email: string
├── password: ?string                  <- NEW (nullable)
├── setup_token: ?string              <- NEW
└── tenant_id: int                     <- FK → tenants

## Affordances

[Per-slice affordance table — extracted from affordances.md]

| # | Component | Affordance | Control | Wires Out | Returns To |
|---|-----------|------------|---------|-----------|------------|
| U1 | ... | ... | ... | ... | ... |

---

### Task 1: [Component Name] (fulfills R0, R1)

**Files:**
- Create: `exact/path/to/file.ext`
- Modify: `exact/path/to/existing.ext`

**Step 1:** [Action — exact code or specific instruction]

**Step 2:** [Verification — concrete proof the step worked]
Verification can be: test command, API call, browser check via /agent-browser, CLI output, or any observable result.

...

### Task N: Validation (fulfills all R's)

Dispatch independent testing agents via /subagents — never self-validate:
- Agent prompts include WHY, may include WHAT, never HOW
- Trace every code path touched to verify correctness
- Validate code serves real user scenarios end-to-end
- Browser testing via tester agent + /agent-browser when UI is involved
- Exhaust every test category fitting scope (unit, integration, user flows, edge cases)
- Autonomously fix all issues that don't significantly change the plan's architecture
- Final step: present results to user for manual verification and feedback (no commits)

---

## Acceptance Criteria
- [ ] [Functional criterion]
- [ ] [Regression criterion]
- [ ] [Dependency audit criterion]
- [ ] [Boundary criterion]
- [ ] [Validation criterion — feature works end-to-end from a user's perspective]

## Changes

[Annotated file tree of all code files created/modified in this slice]

app/
├── Models/Account.php*              <- +password, +setup_token
├── Controllers/AuthController.php*  <- +setPassword(), +sendResetLink()
├── Services/EmailService.php*       <- NEW (extracted from tenant)
admin/
├── pages/SetPassword.tsx*           <- NEW
├── pages/Login.tsx*                 <- +forgot password toggle
```

**Task structure:**
- Bite-sized steps (2-5 min each)
- Exact file paths, complete code
- Each step has concrete verification (not limited to unit tests)
- Pattern: **do → verify → commit**

### 8. Symlink for Plan Mode

After writing V*-plan.md files, create symlinks so Claude Code plan mode can reference them:

```bash
ln -s ~/.claude/shaping/[feature]/V1-plan.md ~/.claude/plans/[descriptive-name].md
```

Claude Code reads/writes through symlinks transparently.

### 9. Validation Checklist

Before presenting the plan to the user, verify:
- [ ] Every R has at least one slice that satisfies it
- [ ] Every slice has all 4 acceptance criteria categories
- [ ] Every modified file has regression criteria
- [ ] Every slice has validation requirements
- [ ] No ⚠️ flags remain on shape parts
- [ ] slices.md and all V*-plan.md files written
- [ ] Symlinks created
- [ ] Every slice passes the demo test (can a user see this working?)

### 9b. Present Requirements & Decisions

Before announcing the contract, present to the user:
- **Full requirements table** — which slice satisfies each R
- **Boundaries (X)** — confirm all respected
- **Outstanding decisions** — anything that emerged during slicing
- **Coverage gaps** — any R without a satisfying slice

The user reviews the plan through conversation output, not by opening slices.md.

### 10. Announce the Contract

After all plan files are written, state:

> "This plan is now an immutable contract. During execution:
> - Architectural changes require explicit approval
> - Tactical code-level adjustments are fine
> - Scope additions must be flagged as must-have or nice-to-have, never silently absorbed"

---

## Plan Contract

The plan defines the architecture. The executing agent implements it. The agent does NOT redesign it.

### What's Fixed (Requires Approval to Change)

- File structure — which files are created, modified, deleted
- Module boundaries — what belongs where, dependency direction
- Patterns — which existing patterns to follow, which abstractions to use
- Data ownership — which component owns which data
- API contracts — function signatures, route shapes, response formats
- Scope — what's included and what's explicitly excluded (boundaries)

### What's Flexible (Agent Decides)

- Variable and function names (within project naming conventions)
- Error message wording
- Internal implementation details (algorithm choice within the same complexity class)
- Test structure (how tests are organized, not what's tested)
- Code comments and documentation

### Deviation Protocol

If an executing agent discovers the plan is wrong or incomplete:

1. **STOP** — do not work around it
2. **State the issue** — "The plan says X, but Y is actually the case because Z"
3. **Propose options** — present alternatives with tradeoffs
4. **Wait for approval** — do not proceed until the architect confirms
5. **Record the deviation** — note in the slice's completion summary

### Scope Additions

During execution, agents may discover work the plan didn't anticipate:

- **Must-have** — blocks the current slice from being demo-able. Flag immediately, propose minimal addition
- **Nice-to-have** — improves quality but slice works without it. Note for later, do not absorb silently
- **Out of scope** — belongs to a different feature or future work. Note in completion summary, do not implement

Never silently absorb scope additions. Every addition is explicitly flagged.
