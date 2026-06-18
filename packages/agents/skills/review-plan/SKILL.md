---
name: review-plan
description: Reviews planning artifacts — shapes, models, slices, and plans. Dispatches 5 parallel specialized agents. Use when reviewing any artifact from the shaping → modeling → slicing pipeline.
---

# Review Plan

Full review gate for planning artifacts. Runs 5 parallel agents against shapes, models, slices, or plans from the `~/.claude/shaping/[feature]/` directory.

## Input

The user provides a feature name or file path. Determine which artifacts exist:

- `shaping.md` — requirements, boundaries, shapes, fit checks
- `affordances.md` — UI/Code affordance tables, data stores, wiring
- `slices.md` — slice definitions with acceptance criteria
- `V*-plan.md` — individual slice implementation plans

Read all available artifacts before dispatching agents. Each agent receives the full content inline.

## Instructions

Launch 5 reviewers in parallel using the Agent tool. Include the full artifact content in each prompt.

### Completeness Agent

Dispatch using `subagent_type: "code-reviewer"`. Prompt:

```
You are reviewing planning artifacts for completeness and accuracy.

## Artifacts
[paste all available artifact content]

## Review Protocol

**Requirements tracing:**
1. List every requirement (R0, R1...) from the shaping doc
2. For each requirement: does a concrete mechanism deliver it? Trace R → shape part → affordance → slice
3. Check for orphan requirements — any R with no corresponding slice
4. Check for orphan slices — any slice with no requirement driving it
5. Check for implicit dependencies — does slice N depend on something not yet built and not in an earlier slice?
6. Check boundaries (X) — does any artifact violate a stated boundary?

**Acceptance criteria:**
7. For each slice: does it have all 4 acceptance criteria categories (functional, regression, dependency audit, boundary)?
8. For each slice: does it have validation requirements?

**Fact-checking:**
9. For every claim about how existing code works (e.g., "X function does Y", "Z table has column W"), use the trace skill to verify against the actual code. Flag any claim that cannot be verified or contradicts what the code shows
10. Check git log --oneline -20 for affected directories. Does a recent commit already solve or partially solve what this plan proposes?

**Assumption audit:**
11. Identify every assertion about existing code behavior. For each: is it backed by a file read, or is it an assumption? Flag unverified assertions as "UNVERIFIED ASSUMPTION: [claim]"
12. Data availability check: for each step, verify that the data/context it needs is actually available at that execution point

**Completeness:**
13. Infrastructure prerequisites: does the plan assume infrastructure that may not exist? Queue workers, cron/scheduler, database tables, env vars, vault secrets, third-party configs
14. Consumer propagation: for every API/interface change, verify ALL consumers are updated — including test files, seed files, and documentation
15. Research depth: for every recommendation, does it cite specific code, library behavior, or user flows? Flag generic pros/cons without code-level evidence
16. Gap detection: identify sections with "TBD", deferred decisions, or options without recommendations — each is a gap requiring human input
17. Scope proportionality: compare the plan's scope to the original ask. Flag new abstractions, files, or patterns beyond what was requested as "SCOPE EXPANSION: [what was added]"

Report using Critical/Important/Minor format.
If clean: "All requirements covered. All claims verified."
```

### UX Flows Agent

Dispatch using `subagent_type: "ux-tester"`. Prompt:

```
You are reviewing planning artifacts for UX completeness. Can a real user actually USE everything being built?

## Artifacts
[paste all available artifact content]

## Review Protocol

1. Identify every new feature, page, or interaction the plan introduces
2. For each: trace the complete user journey
   - Entry point: how does the user GET to this? Is there a link, button, menu item?
   - Steps: what does the user do at each step? What do they see?
   - Exit: where does the user end up? Is the end state clear?
3. Check for dead ends — features built but unreachable from existing UI
4. Check for missing error states — what happens when things fail?
5. Check for missing empty states — what does the user see before any data exists?
6. Check for missing loading states — what does the user see while data loads?
7. Reachability gate: for every new feature or page, verify there is an explicit plan to make it reachable from existing UI. A feature with no navigation path is dead on arrival

Report using Critical/Important/Minor format.
If clean: "All user flows are complete and reachable."
```

### Regressions Agent

Dispatch using `subagent_type: "code-reviewer"`. Prompt:

```
You are reviewing planning artifacts for regression risk. For every file the plan modifies, determine what could break.

## Artifacts
[paste all available artifact content]

## Review Protocol

For every file listed as modified in the plan:

1. Read the actual file in the codebase
2. Identify all existing behavior — what does this file currently do? What depends on it?
3. Use the trace skill to find all callers/importers of modified functions, classes, components, or exports
4. For each existing behavior: does the plan's change preserve it? Could it break?
5. Check: are there existing tests? Does the plan include regression checks in acceptance criteria?
6. Check: does the plan modify shared utilities, base classes, or interfaces? If so, trace ALL consumers
7. Bidirectional impact: for every fix or change, check the reverse direction. If fixing A→B interaction, verify B→A still works. Especially for auth/cookie changes, shared state, platform/tenant boundaries
8. Cross-system cascades: does the modified code run inside try-catch blocks owned by other systems? Could a failure here silently prevent other listeners/hooks/middleware from executing?
9. Migration completeness: for any migration, renaming, or deletion, use the trace skill across the ENTIRE codebase for references to old names, old paths, or old values. Include config files, vault, CI/CD, deployment scripts

Report using Critical/Important/Minor format.
If clean: "No regression risks identified."
```

### Architect Agent

Dispatch using `subagent_type: "architect"`. Prompt:

```
You are reviewing planning artifacts for architectural quality.

## Artifacts
[paste all available artifact content]

## Review Protocol

**Existing patterns:**
1. Read the nearest Claude.md files for the affected codebase — these define conventions, patterns, and boundaries
2. For each new file/component the plan creates:
   - Does something similar already exist? Search the codebase
   - Does it follow existing patterns in the same directory?
   - Is naming consistent with project conventions?
3. Check encapsulation: are module boundaries respected? Does the plan reach into internals?
4. Check dependency direction: any circular or wrong-direction dependencies?
5. Check code reuse: does the plan duplicate functionality that already exists?
6. Cross-reference against Claude.md requirements and boundaries — any violations?

**Model quality (if reviewing affordances):**
7. Trace user stories through the wiring — does the path tell a coherent story?
8. Check for design smells: incoherent wiring, missing paths, diagram-only nodes, naming resistance, stale affordances, wrong causality, implementation mismatch
9. Apply the Naming Test to each affordance:
   - Who is the caller?
   - What is the step-level effect (not the downstream chain)?
   - Can you name it with ONE idiomatic verb?
   - If you need "or" to connect two verbs → likely two affordances bundled
   - If the name matches a downstream effect → you're naming the chain, not the step

**Feasibility and decisions:**
10. Library verification: for every external library/package the plan uses, verify the API surface matches what the plan assumes. Check that methods, parameters, and behaviors actually exist
11. Decision completeness: list every architectural decision the plan makes. For each: is it made and justified, or deferred to implementation? Flag deferred decisions as "DEFERRED: [decision]"
12. Foundation-first ordering: does the plan build foundational/risky pieces first? If a later slice could invalidate earlier ones, flag as "ORDERING RISK: Slice N depends on unvalidated approach in Slice M"

**Behavioral changes:**
13. Flag any change that alters observable behavior (error handling, redirect targets, response shapes, auth flows) even if described as a "refactor" or "cleanup". Mark as "BEHAVIORAL CHANGE: [description]"

Report using Critical/Important/Minor format.
If clean: "Architecture is sound."
```

### Frontend Agent

Dispatch using `subagent_type: "frontend-engineer"`. Prompt:

```
You are reviewing planning artifacts for frontend architecture and implementation quality.

## Artifacts
[paste all available artifact content]

## Review Protocol

**Architecture:**
1. Read the nearest Claude.md files for frontend conventions
2. Read 3+ existing components in the same directory/module as the plan's new components
3. Check frontend architecture: component hierarchy, state management patterns, data flow patterns, routing conventions
4. Check for component reuse — does the plan create new components where existing ones could be extended?
5. Check data flow — are API calls, queries, mutations following project patterns?

**Component quality:**
6. For each new frontend component:
   - Does it follow existing component patterns? (naming, file structure, props, CSS approach)
   - Is state management consistent with similar components?
   - Are interaction patterns consistent with existing UI?
7. Check for missing interactive states: hover, focus, active, disabled

Report using Critical/Important/Minor format.
If clean: "Frontend architecture and patterns are consistent."
```

## Aggregation

After all 5 agents complete:

```
# Plan Review

## Critical
[issues from all agents, prefixed with agent name]

## Important
[issues from all agents, prefixed with agent name]

## Minor
[issues from all agents, prefixed with agent name]
```

If all clear: "No issues found."

## Gate

- **Critical:** Block. Artifacts must be revised.
- **Important:** Report and ask user.
- **Minor:** Report and continue.
