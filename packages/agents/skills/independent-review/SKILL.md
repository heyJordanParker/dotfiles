---
name: independent-review
description: Run N identical parallel Subagents on the same Task for independent Verification. Use when the Architect wants consensus through redundancy — multiple Subagents independently analyze, review, or test the same thing, then results are compared for agreement, disagreements, and unique finds. Invocation /independent-review [count] "task"
disable-model-invocation: true
---

# Independent Review

- N identical Subagents do the same Task independently.
- The value is consensus through redundancy: agreement, disagreements, and unique finds.

## 1. Capture current changes for Review Tasks

Current Changes:

!`git changes`

Full Diff:

!`git diff HEAD`

## 2. Parse the Architect's input

The first numeric argument is the Subagent count. Default to three Subagents when no count is provided. Everything else is the Task.

Example: `/independent-review "is this migration safe?"` runs three Subagents.

Example: `/independent-review 5 "review for security vulnerabilities"` runs five Subagents.

## 3. Build one Prompt

Every Subagent receives the exact same Prompt with no variation.

Write the Prompt with /subagents. What this Skill adds to that Template:

    Goal: perform this analysis independently. Document every finding with Evidence —
    file paths, line numbers, concrete examples. State conclusions directly.

    Verification:
    - Every finding includes Evidence, not just assertions.
    - Conclusions are stated directly, not hedged.
    - Output is structured with clear sections.

    Architecture: the Task scope. For a Review of code changes, use Current Changes and
    Full Diff above; mark files to inspect with `*`.

### Keep Subagent inputs identical

Use the same `subagent_type`, Prompt, and tools for every Subagent. Reasoning variation provides the natural diversity.

Never: assigning different lenses, files, or specialties to each Subagent.

## 4. Dispatch N Subagents in parallel

Dispatch every reviewer in one message, per /subagents. Each returns its report on its own; collect all of them before synthesizing.

### Keep the Skill Task neutral

This Skill wraps any Task the Architect provides: code Review, bug analysis, Architecture assessment, test adequacy, or migration risk.

## 5. Synthesize the results

Compare all Subagent returns before reporting.

- Consensus means findings that two or more Subagents independently identified. List each finding and which Subagents found it.
- Unique finds means findings only one Subagent caught. These need Architect judgment because each could be an insight or a false positive.
- Disagreements means Subagents contradict each other. Present both sides with Evidence.
- Confidence means the `N-of-N` agreement ratio, such as "3/3 Subagents agree" or "2/5 Subagents found this".

### Require Evidence for every finding

Every finding includes concrete Evidence. "Might be a problem" is not a finding.
