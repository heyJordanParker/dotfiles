---
name: codex
description: Drive codex CLI runs as your Agents — you own Orchestration, codex does the work. One wrapper, `codex-run`, owns the mechanics (flags, output storage, stream parsing, failure detection); you write the Task Prompt and do the judgment. Calls /subagents for Orchestration doctrine on a codex-run substrate. TRIGGER when the Architect says "codex", "/codex", "codex-run", "use codex", "dispatch to codex", "run this through codex", "codex agents", "review with codex", "codex review", or asks to fan out work across codex runs. DO NOT TRIGGER for native Claude Code Subagents (use /subagents) — those run inside this Harness; codex is a separate CLI process.
---

# codex

Codex runs are Agents in a separate Harness.
The Orchestrator holds the Goal and Orchestration; each codex run does the work and returns.
`codex-run` owns flags, output storage, stream parsing, and failure detection; the Orchestrator writes the Prompt and does the judgment.

## 1. Compose through /subagents

### Follow the Subagent Process
Use /subagents for the Prompt Template, WHAT/WHY not HOW, independent dispatch, claim handling, Verification, the long hard way, and the ranking Process. Codex only changes the Execution substrate.

### Let the Agent type carry the lens
There is no review mode. The lens lives in the Agent type's Prompt; scope the Prompt to what to review.

Example: `codex-run @code-reviewer "review the uncommitted changes"`.

## 2. Start each run through codex-run

### Use the wrapper as the whole interface
`codex-run` owns the no-sandbox run, event stream parsing, output storage, and failure detection. Never parse its output, pipe it, or touch the event stream by hand.

Template:
  codex-run @<agent> "<prompt>"        # run codex as <agent>, return its final answer
  codex-run resume <session> "<msg>"   # continue a prior run with full Context

### Use a named Agent type
`@<agent>` resolves to that Agent type's own Prompt. An unknown Agent type exits non-zero and lists the available ones, so a typo is self-correcting.

### Run every codex-run in the background
A foreground run blocks Orchestration on the codex turn; a guard blocks it otherwise. Use `run_in_background: true` and read the result when it lands.

Never: a foreground codex-run.

## 3. Read the final answer as a claim

### Treat the wrapper output as the summary
The claim is the wrapper's final answer, not a Claude Subagent summary. A zero exit means the turn completed; the answer is still a claim you must prove.

### Check the trailer
Parallel runs get collision-free output paths automatically. The wrapper exits non-zero on failure: a Process failure or a failed turn, even one that says "done".

Template:
  <the final answer codex produced>
  --- codex-run ---
  status:  ok            # or "failed"
  session: <id>          # the run's identity; pass to `codex-run resume`
  output:  <path>        # the final answer on disk, in this session's own directory
  events:  <path>        # the raw event stream on disk

## 4. Iterate with the same run

IF the codex run needs iteration:
### Resume the same session
Use `codex-run resume <session> "<msg>"` with feedback so the run keeps full Context. Do not re-explain to a fresh run.

## 5. Close research gaps

### Re-dispatch thin work
Re-dispatch until every claim comes from a read. Stop only when no gap remains.

## 6. Verify the work

### Prove behavior against the repo
Check the repo or exercise the User-facing Critical Path.

### Do not edit while orchestrating
Every line of code is written by a codex run.

Never: Edit, Write, or NotebookEdit while orchestrating.

## 7. Synthesize yourself

### Use the /subagents ranking Process
Strip codex-run recommendations, keep facts, rank with /pcc, and present your own judgment.

Never: forwarding a run's ranking or recommendation.
