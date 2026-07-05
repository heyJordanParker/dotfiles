---
name: review
description: Run the full code review gate on uncommitted changes by dispatching the reviewer Subagents in parallel, aggregating Critical, Important, and Minor findings, and blocking on Critical. TRIGGER on "/review", "code review", "review the changes", or before commit when the Architect asks for review.
disable-model-invocation: true
---

# Review

- Full code Review gate for uncommitted changes.
- The gate runs eight reviewer Subagents in parallel through the Agent tool, not the Task tool.

## 1. Capture the current changes

Current Changes:

!`git changes`

Full Diff:

!`git diff HEAD`

### Paste the captured diff into code-reading Subagent Prompts

Use the Current Changes and Full Diff blocks above instead of telling Subagents to run `git diff HEAD`.

## 2. Dispatch the six diff reviewers

Dispatch these six Subagents in parallel. Each Prompt includes the Current Changes and Full Diff blocks from step 1.

Use each backticked name as the `subagent_type`; the general-purpose naming Subagent uses no `subagent_type`.

Template:
    `code-reviewer` Prompt:
    Review uncommitted changes. Review the diff provided and scan all 12 AI Slop categories. Report using Critical/Important/Minor format. If clean: "No AI Slop found."

Template:
    `architect` Prompt:
    Review uncommitted changes. Review the diff provided and apply your full review protocol. Report using Critical/Important/Minor format.

Template:
    general-purpose naming Prompt:
    Apply the /naming Skill. Review uncommitted changes. Review the diff provided and check all changed identifiers for naming issues. Report using Critical/Important/Minor format. If clean: "No naming issues found."

Template:
    `backend-engineer` Prompt:
    Review uncommitted changes. Review the diff provided and apply your review-mode protocol: reinvented wheels, library leverage, unnecessary complexity, approach quality, and public contract regressions. Report using Critical/Important/Minor format. If clean: "Code is appropriately simple."

Template:
    `frontend-engineer` Prompt:
    Review uncommitted changes. Review the diff provided and apply your Critical Path protocol. Identify affected Critical Path, trace each through the code, and report gaps. Report using Critical/Important/Minor format. If clean: "All Critical Path verified."

Template:
    `regression-reviewer` Prompt:
    Review uncommitted changes for capability regressions. Map the diff to affected Critical Path and system capabilities. Trace each end-to-end. Flag any capability that is lost or degraded. Report using Critical/Important/Minor format. If clean: "No capability regressions found."

## 3. Dispatch the documentation reviewer

Dispatch `context-engineer` with the Current Changes and Full Diff blocks from step 1.

Template:
    `context-engineer` Prompt:
    Audit Claude.md files against the current uncommitted changes. Report Critical/Important/Minor findings.
    Critical: Architectural change with no Claude.md, or Claude.md Rules contradicted by changes.
    Important: missing WHY for a significant Decision, stale Rule, or wrong hierarchy placement.
    Minor: Template compliance gaps, Fluff, or pruning opportunities.
    If clean: "Documentation is up to date."
    Do not make changes. Report findings only.

## 4. Dispatch the User experience reviewer when it can test a Critical Path

IF no development server is running, changes are backend-only with no User Interface impact, or affected routes cannot be determined from the diff:
### Skip the `ux-tester` Subagent

Do not dispatch a User experience reviewer when there is no reachable Critical Path to test.

When the Condition does not apply, dispatch `ux-tester` without the diff. Translate the diff into features and Critical Path, with URLs or routes when identifiable.

Template:
    `ux-tester` Prompt:
    Do a complete User experience Review of the following features and Critical Path affected by the current changes: [list features and Critical Path derived from the diff, with URLs or routes if identifiable].
    Report Critical/Important/Minor findings.
    If clean: "User experience is clean."

## 5. Aggregate findings

Merge duplicate findings and keep the highest severity.

Template:
    # Code Review

    ## Critical
    [issues]

    ## Important
    [issues]

    ## Minor
    [issues]

If all clear: "No issues found."

## 6. Apply the gate

Critical blocks. Do not proceed.

Important is reported to the Architect.

Minor is reported and does not block.
