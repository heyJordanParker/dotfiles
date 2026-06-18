---
name: independent-review
description: Run N identical parallel subagents on the same task for independent validation. Use when you want consensus through redundancy — multiple agents independently analyze, review, or test the same thing, then results are compared for agreement, disagreements, and unique finds. Invocation /independent-review [count] "task"
---

# Independent Review

N identical agents do the same work independently. Compare results for consensus.

## Current Changes

!`git changes`

## Full Diff

!`git diff HEAD`

## Triggers

- "independent review", "cross check", "validate independently"
- "run N agents on this", "get consensus", "replicate this analysis"
- Any task where independent validation adds confidence

## Process

1. **Parse input** — Extract agent count and task from user input.
   - `/independent-review "is this migration safe?"` — 3 agents (default)
   - `/independent-review 5 "review for security vulnerabilities"` — 5 agents
   - Count is always the first argument if numeric. Everything else is the task.

2. **Build the prompt** — Write ONE prompt. Every agent gets this exact prompt with no variation. Follow the prompt structure (Story/Business/Goal/DoD):

```
Story: {task — what the user wants analyzed/reviewed/tested and why}

Business: {constraints — codebase context, stack, what matters}

Goal: Perform this analysis independently. Be thorough. Document every
finding with evidence (file paths, line numbers, concrete examples).
Do not hedge — state your conclusions directly.

DoD:
- Every finding includes evidence (not just assertions)
- Conclusions are stated directly, not hedged
- Output is structured with clear sections
```

3. **Dispatch N identical agents in parallel** — Same `subagent_type`, same prompt, same tools. Use `run_in_background: false` so all results are collected. Name agents `reviewer-1`, `reviewer-2`, etc.

4. **Synthesize** — After all agents return, compare results:

   - **Consensus** — Findings that 2+ agents independently identified. These are high-confidence. List each finding and which agents found it.
   - **Unique finds** — Things only 1 agent caught. These need human judgment — could be an insight others missed, or a false positive.
   - **Disagreements** — Where agents contradict each other. Present both sides with their evidence.
   - **Confidence** — `N-of-N` agreement ratio (e.g., "3/3 agents agree" or "2/5 agents found this").

## Key Rules

- **Identical agents** — same prompt, same tools, same model. No differentiation. Temperature and reasoning variation provide natural diversity.
- **No use-case limits** — this skill wraps any task. Code review, bug analysis, architecture assessment, test adequacy, migration risk — whatever the user provides.
- **Evidence required** — every finding must include concrete evidence. "Might be a problem" is not a finding.
