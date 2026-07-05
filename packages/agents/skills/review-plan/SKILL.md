---
name: review-plan
description: Reviews Shaping, Modeling, Slicing, and Plan Prompts under docs/shaping/[feature]. Dispatches five parallel specialized Subagents. TRIGGER when the Architect asks to review a Shaping, Modeling, Slicing, or Plan artifact.
disable-model-invocation: true
---

# Review Plan

- Full Review gate for Shaping, Modeling, Slicing, and Plan Prompts under `docs/shaping/[feature]`.
- Each Subagent receives the same artifact content and reports only Critical, Important, or Minor findings.

## 1. Read the artifacts

The Architect provides a feature name or file path. Determine which files exist and read every available artifact before dispatching Subagents.

- `shaping.md` contains requirements, boundaries, shapes, and fit checks.
- `affordances.md` contains User Interface Affordances, Code Affordances, data stores, and wiring.
- `slices.md` contains Slice definitions with acceptance criteria.
- `V*-plan.md` contains Slice implementation Plans.

### Every reviewer receives the same artifact content

Paste the full content from every available artifact into each Subagent Prompt.

## 2. Build the five reviewer Prompts

Use [agent-prompts.md](references/agent-prompts.md) to build the five review-plan Subagent Prompts.

- Completeness uses `subagent_type: "code-reviewer"` and checks requirement tracing, acceptance criteria, code-claim Verification through `/trace`, assumptions, infrastructure prerequisites, consumer propagation, research depth, gaps, and scope proportionality.
- Critical Path uses `subagent_type: "ux-tester"` and checks whether every new User-facing feature has an entry point, steps, exit, states, and reachability.
- Regressions uses `subagent_type: "code-reviewer"` and checks existing behavior, callers through `/trace`, preservation, tests, shared utilities, bidirectional impact, cross-system cascades, and migration completeness.
- Architect uses `subagent_type: "architect"` and checks Precedents, Claude.md Rules, encapsulation, dependency direction, reuse, Modeling quality, library Verification, Decision completeness, ordering, and behavior changes.
- Frontend uses `subagent_type: "frontend-engineer"` and checks frontend Architecture, Claude.md Rules, sibling components, reuse, data movement, component quality, and interactive states.

## 3. Dispatch five Subagents in parallel

Launch all five reviewer Subagents through the Agent tool with the Prompts from step 2.

## 4. Aggregate the findings

Prefix each issue with the Subagent name and group by severity.

Template:
    # Plan Review

    ## Critical
    [issues from all Subagents, prefixed with Subagent name]

    ## Important
    [issues from all Subagents, prefixed with Subagent name]

    ## Minor
    [issues from all Subagents, prefixed with Subagent name]

If all clear: "No issues found."

## 5. Apply the gate

Critical blocks until artifacts are revised.

Important is reported to the Architect.

Minor is reported and does not block.
