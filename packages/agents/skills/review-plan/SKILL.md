---
name: review-plan
description: Reviews Shaping, Modeling, Slicing, and Plan Prompts under docs/shaping/[feature]. Dispatches five parallel specialized Subagents. TRIGGER when the Architect asks to review a Shaping, Modeling, Slicing, or Plan artifact.
---

# Review Plan

- Full Review gate for Shaping, Modeling, Slicing, and Plan Prompts under `docs/shaping/[feature]`.

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

## 3. Dispatch five Subagents in parallel

Launch all five reviewer Subagents through the Agent tool with the Prompts from step 2.

## 4. Aggregate the findings and apply the gate

Use /triage for the aggregation Template and the gate. Title the report `# Plan Review` and prefix each issue with the Subagent name that found it.

### Block until the artifacts are revised
Critical here blocks the artifact, not a commit: the Shaping, Modeling, Slicing, or Plan Prompt is revised before Execution starts.
