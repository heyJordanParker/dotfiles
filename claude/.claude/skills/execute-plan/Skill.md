---
name: execute-plan
description: Orchestration SOP for executing implementation plans. Assigns slices to a persistent team, runs validation gates per slice. The orchestrator coordinates — it does not implement.
---

# Execute Plan

Orchestration SOP for the main thread agent. You coordinate a persistent team to implement a plan slice by slice. Use the /subagents skill — create the team at the start using specialized agent types (backend-engineer, frontend-engineer, architect, context-engineer, etc.), then assign slices to teammates sequentially. Do not create custom agents — use the existing specialized agents. You do NOT read full implementation files. You do NOT use Edit, Write, or NotebookEdit. Every line of code is written by a teammate.

## Triggers

- A plan file exists and is ready for execution (V*-plan.md)
- User says "execute the plan", "implement", "build this", or similar after planning is complete

## Prerequisites

Before executing, verify:
- Plan file exists and has been approved by the user
- Plan has slices with acceptance criteria
- Plan has a WHY section

If any prerequisite is missing, stop and tell the user what's needed.

## The Immutable Contract

The plan defines the architecture. The executing agent implements it. The agent does NOT redesign it.

**What's Fixed (Requires Approval to Change):**
- File structure — which files are created, modified, deleted
- Module boundaries — what belongs where, dependency direction
- Patterns — which existing patterns to follow, which abstractions to use
- Data ownership — which component owns which data
- API contracts — function signatures, route shapes, response formats
- Scope — what's included and what's explicitly excluded

**What's Flexible (Agent Decides):**
- Variable and function names (within project naming conventions)
- Error message wording
- Internal implementation details (algorithm choice within same complexity class)
- Test structure (how tests are organized, not what's tested)
- Code comments and documentation

**Scope Additions:**
- **Must-have** — blocks the current slice. Report to user immediately, wait for approval
- **Nice-to-have** — improves but doesn't block. Log it, report in slice summary, do not implement
- **Out of scope** — belongs to different feature or future work. Note in completion summary, do not implement

Never silently absorb scope additions.

## Hard Rules

- You do NOT use Edit, Write, or NotebookEdit tools
- You do NOT read full implementation files — read summaries, spot check selectively
- You do NOT trust teammate success reports — verify independently via a separate validation agent (fresh, not part of the team)
- Every slice gets validation before staging
- Architectural deviations halt execution until the user approves
- Use TaskCreate for each slice with `activeForm` in present-continuous (e.g., "Implementing password reset slice") — gives the user real-time progress via spinners and checkmarks
- TaskUpdate to completed on success, or to failed on halt — never leave tasks hanging
- Sequential slices only — later slices depend on changes from earlier slices; parallel execution creates merge conflicts and ordering bugs

## Process

### Phase 0: Readiness

Before any implementation begins:

**1. Run the test suite** to establish a baseline. Record the result — any pre-existing failures must be noted so validation can distinguish new regressions from known issues.

**2. Dispatch via Agent tool (`subagent_type: "context-engineer"`):**

```
Story: We're about to execute an implementation plan. The Claude.md
files need to reflect the architectural decisions, WHY, philosophy,
requirements, and boundaries from the plan before subagents start
working.

Business: Subagents read Claude.md files for context. If these files
don't capture the plan's intent, subagents will make wrong assumptions
— the #1 failure mode from our retro (31 of 182 failures).

Goal: Read the plan file at [path] and the shaping doc at [path].
Update the relevant Claude.md files so that a subagent reading them
understands the WHY, requirements, boundaries, and architectural
patterns for this work.

DoD:
- Relevant Claude.md files updated with WHY from the plan
- Requirements and boundaries from the plan reflected in docs
- Architectural patterns and conventions documented
- No fabricated WHY — only what the plan and shaping docs establish

[Annotated file tree of Claude.md files relevant to this plan's scope]
```

**3. Spot-check** the Claude.md changes before proceeding.

**4. Verify readiness:** dev server starts, database accessible, required services running. If the plan introduces new infrastructure (queue workers, env vars), verify those exist before the slice that depends on them. If readiness fails, halt and report to the user — do not proceed with broken infrastructure.

### Phase 1: Per-Slice Execution (Sequential)

For each slice in the plan, in order:

**TaskCreate** for this slice. **Report to user:** "Starting Slice N/M: [slice name]"

#### Step 1a: Dispatch Implementing Subagent

Use the /subagents skill. Dispatch teammates — the team persists across slices, sharing context and learnings:

```
Story: [What the user will experience when this slice is done — from
the plan's slice description and demo line]

Business: [WHY from the plan. What problem this solves. What
constraints exist.]

[Weave in slice learnings from previous slices — a library limitation
goes in Business, a broken test goes in Story]

Goal: Implement [slice name] as defined in the plan. Read all files
marked * in the Changes section. The plan is an immutable contract
for architecture — adapt tactically to what you find in the code,
but do not change the architectural approach.

BEFORE IMPLEMENTING: Produce an Assumption Audit:
- List every assumption the plan makes about the code you just read
- For each: CONFIRMED (with evidence) or WRONG (what's actually true)
- If any assumption is WRONG, stop and report to the orchestrator

DoD:
[Paste the slice's acceptance criteria from the plan, verbatim]
- All acceptance criteria verified with evidence (command output, not assertions)
- Report any tactical deviations made and why
- Flag any change to user-visible behavior, error handling, or auth flows as BEHAVIORAL CHANGE
- For any file deletion, rename, or moved symbol: trace all references and report the chain

[Annotated file tree from the plan's Changes section, with * marking files to read]

Workflow:
1. Read every file marked * above
2. Produce Assumption Audit — stop if any assumption is WRONG
3. Implement against the Goal
4. For EACH DoD item: run verification, paste relevant output
5. If any DoD item fails → fix and re-verify (loop step 4)
6. Post completion summary: what changed, what was verified, what was tricky, slice learnings for future slices
```

#### Step 1b: Validate the Slice

After the implementing subagent reports completion:

1. **Run the test suite** — compare against Phase 0 baseline. If no test command is known, ask the user once. Do not trust "tests pass" without evidence.

2. **Dispatch a fresh validation agent** (NOT part of the persistent team — fresh context prevents bias from the implementing teammate):

```
Story: Slice [N] of [plan name] was just implemented. We need to
verify it meets its PURPOSE (not just its criteria) and doesn't
regress existing behavior.

Business: Agent success reports are unreliable. Independent validation
catches gaps that self-reported DoD misses. Criteria can be incomplete
— the purpose check catches what criteria miss.

Goal: Validate this slice against its acceptance criteria, its stated
purpose, and check for regressions and cross-module interactions.
If UI changes were made, use /agent-browser to verify visually.

DoD:
- Verify each acceptance criterion across all 4 categories:
  functional, regression, dependency audit, boundary
- Purpose check: does this slice achieve the PURPOSE stated in the
  WHY/Story, not just the listed criteria?
- Cross-module check: what other modules interact with modified code?
  For each interaction point, does the change create a new failure path?
- Browser test: if slice has UI changes, use /agent-browser to verify.
  If dev server isn't running, start it or report the blocker — never skip
- Gaps: anything not covered by criteria that broke or degraded

[Annotated file tree from the plan's Changes section]

Workflow:
1. Read every file marked * above
2. Verify each acceptance criterion with evidence
3. Check purpose — does the result match the WHY?
4. Run regression checks per file
5. Check cross-module interactions
6. If UI changes: use /agent-browser to verify visually
7. Post validation report
```

**Validation report format:**
```
## Slice [N] Validation

### Purpose Check
- [Does the slice achieve its stated purpose? PASS/FAIL — evidence]

### Acceptance Criteria
- Criterion 1: PASS — [evidence]
- Criterion 2: FAIL — [what's wrong]

### Regressions
- [file]: [existing behavior] — PASS/FAIL

### Cross-Module Interactions
- [module]: [interaction] — [impact assessment]

### Browser Test (if applicable)
- [page/feature]: [result]

### Gaps
- [anything found not covered by criteria]
```

3. **Spot-check** at least one PASS result from the validation report by reading the evidence yourself. If the evidence doesn't support the claim, re-dispatch the validation subagent.

4. **If validation fails** — dispatch a fix subagent with the specific failures. Re-validate after the fix. **Maximum 3 fix attempts.** If still failing after 3, halt and report: (1) what each attempt tried, (2) why each failed, (3) root cause theory, (4) proposed alternatives.

5. **If failure reveals an architectural issue** — halt execution immediately per the Deviation Protocol below.

#### Step 1c: Stage and Report

After validation passes:
- Stage the changes (`git add`)
- **Report to user:** what was done, what was verified, tactical deviations, behavioral changes flagged, scope additions, slice learnings
- **TaskUpdate** slice to completed

### Phase 2: Final Validation

After all slices are staged:

1. **Full /review** — all changes are staged but uncommitted, so /review's `git diff HEAD` captures everything naturally

2. **Full /user-testing** — enumerate user flows affected by the combined changes and present the flow list to the user for approval (respecting /user-testing's approval gate). Then dispatch subagents per approved flow

3. **Context engineer ledger update** — dispatch the context-engineer agent to update ledgers with what was built and any architectural decisions that emerged

**Report to user:** Final summary — all slices, all validation results, scope additions logged, behavioral changes flagged, overall status.

**Do NOT close the team.** The team persists after all work is done — the user will need teammates to fix issues, iterate, and handle follow-ups. Closing the team wastes all the context teammates have built up.

## Deviation Protocol

When a subagent reports that the plan's architecture won't work:

1. Stop execution
2. Report to the user:
   - What the plan prescribed
   - What the subagent found in the code
   - Why the prescribed approach doesn't work
   - What the subagent recommends instead
3. Wait for the user to approve the deviation or provide alternative direction
4. If approved, update the plan file to reflect the change (so future slices see it)

## Quick Reference

- **Readiness first** — test baseline, context engineer, environment check
- **Sequential slices** — never parallelize slice execution
- **Assumption audit** — subagent verifies plan assumptions before implementing
- **Report at every transition** — user sees progress throughout
- **Validate every slice** — test suite + independent validation + purpose check + browser test if UI
- **Cross-module check** — validation subagent checks interaction points beyond modified files
- **Spot-check validation** — orchestrator verifies at least one PASS claim
- **Trust nothing** — verify subagent claims against actual output
- **3 fix attempts max** — then halt with structured report
- **Flag behavioral changes** — subagents must surface any user-visible behavior change
- **Flag scope** — must-have (halt), nice-to-have (log), out of scope (note)
- **Halt on architecture** — any deviation stops execution
- **Stage and report** — never commit; the user initiates commits when ready
- **Slice learnings** — propagate discoveries between slices
- **Never close the team** — team persists for follow-ups and fixes
- **Specialized agents only** — use existing agent types, never create custom agents
