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
`codex-run` owns the no-sandbox run, event stream parsing, output storage, and failure detection, and its own commands are how a finished run is read back.

Write one invocation per Bash call, spelled out in full. zsh does not word-split an unquoted expansion, so a variable holding a whole command arrives as a single argument and the call falls through to usage without running anything.

Every `<job>` is a leading prefix of the job id, which always begins `codex-run-`. A bare number is not a prefix and matches nothing.

Example: `codex-run result codex-run-59429`.
Never: `codex-run result 59429`, `for c in …; do codex-run $c; done`, piping it, parsing its output, or opening its files by hand.

Template:
  codex-run @<agent> "<prompt>"       # run codex as <agent>, return its final answer
  codex-run resume <job> "<msg>"      # continue a prior run with full Context
  codex-run status [--all]            # every job with its Agent, status, phase, and age
  codex-run result <job>              # a run's final answer with its trailer
  codex-run log <job>                 # what a run did: commands, edits, tool calls, searches
  codex-run events <job> [--tail N]   # the raw event stream
  codex-run history <job>             # codex's rollout path and this session's Claude transcript
  codex-run cancel <job>              # interrupt a running turn, then stop its process tree
  codex-run watch                     # follow this session's lifecycle feed

### Send the Prompt inline through stdin
`-` reads the Prompt from stdin, so a heredoc carries quotes, backticks, `$`, and newlines through untouched. The argv form mangles or kills a run carrying them.

Template:
  codex-run @<agent> - <<'EOF'
  <the whole Prompt, any length, any characters>
  EOF

  codex-run resume <job> - <<'EOF'
  <the whole message>
  EOF

Never: writing the Prompt to a file and redirecting it in.

### Use a named Agent type
`@<agent>` resolves to that Agent type's own Prompt. An unknown Agent type exits non-zero and lists the available ones, so a typo is self-correcting.

### Run every codex-run in the background
A foreground run blocks Orchestration on the codex turn. Use `run_in_background: true` and read the result when it lands.

Never: a foreground codex-run.

IF you are running AS a Subagent:
### Run codex-run foreground, chunked, with your turn kept alive
The opposite holds inside a Subagent: your final message kills in-flight children, so run each codex-run foreground, sized to finish inside the Bash ceiling, and end your turn only after reading every result (see /subagents).

### Run the same Agent on both Harnesses when two perspectives are worth more than one
Every Agent you can dispatch as a Subagent runs on either Harness under one definition: by name, or through `codex-run @<name>`. Both in one message gives two independent workers on the same Task, and the trailer tells the results apart — the codex answer carries one, the Subagent's does not.

Example: `Agent(subagent_type: "ponytail")` and `codex-run @ponytail "<same task>"` in the same message.

### Expect the active profile's Agents on top of the shared roster
`codex-run` resolves `@<name>` against the active config root's Agents first and the shared roster second, so a profile's own Agents are runnable while that profile is active and the shared roster stays reachable behind them. A name both hold runs as the profile's, the same definition that governs a Subagent dispatch. `codex-run` lists what it can run when a name misses.

### Expect a run to have no Memory when its Agent declares it
An Agent whose definition declares `memory: none` runs without Memory, on a founding run and on a resume alike. Give such a run everything it needs in the Prompt — it recalls nothing from earlier work.

IF a run should stop before it finishes:
### Cancel it through the wrapper
`codex-run cancel <job>` interrupts the turn first, so codex finishes the write it is inside instead of leaving a half-applied edit, and stops the process tree only if that does not land.

Never: abandoning an unwanted run — it keeps working and keeps editing.

## 3. Read the final answer as a claim

### Let the feed report completion instead of polling
`codex-run watch` follows this session's lifecycle feed, sized for a Monitor to read: one line when a job starts and one when it ends, carrying the status, the elapsed time, and the size of the answer on disk. `codex-run status [--all]` prints the whole board once, where a job whose runner died reads `failed` — reading a record reconciles it, so a dead run never sits at `running`.

Never: repeated `status` calls to see whether a run has finished.

### Arm the feed once, from the Orchestrator
At your first dispatch, arm `codex-run watch` as a persistent Monitor and never arm a second one. Every run in the session appends to the same feed, so that one Monitor carries the runs you dispatch later too, while a second doubles every line toward the rate limit that stops a Monitor silently — ten lines burst, then one every two seconds. A Monitor's events reach only the Agent that armed it, so a Subagent that arms one gets a feed that dies with its turn (see /subagents).

`codex-run watch` owns its own cadence: two lines per job, nothing in between, because every line it prints wakes you for a full turn. A run that dies without writing its terminal line reaches you at the end of the turn instead, when the failure rewake fires.

Template:
  Monitor(command: "codex-run watch", description: "codex jobs", persistent: true)

Never: a second `watch`, arming one from a Subagent, or a poll interval of your own around codex jobs.

IF you are dispatching and no Monitor is armed:
### Arm it on this message
A missed first dispatch is not retroactively unsatisfiable. The one feed holds every run in the session, so arming it late still carries every run from here — a session that got this far without one is behind, not disqualified.

### Fetch a result on demand
`codex-run result <job>` prints any run's answer with its trailer, and `codex-run log <job>` prints what the run did — every command, edit, tool call, and search. Both read from disk long after the run ended, so no result is lost by not watching for it.

### Treat the wrapper output as the summary
The claim is the wrapper's final answer, not a Claude Subagent summary. A zero exit means the turn completed; the answer is still a claim you must prove.

### Check the trailer
Parallel runs get collision-free output paths automatically. The wrapper exits non-zero on failure: a Process failure or a failed turn, even one that says "done".

Template:
  <the final answer codex produced>
  --- codex-run ---
  status:  ok                        # ok, failed, or cancelled
  agent:   <name>                    # the Agent the run ran as
  model:   <name>  (effort <level>)  # what it ran on, from the Agent's own declarations
  job:     <id>                      # the run's identity; pass to resume, result, log, cancel
  thread:  <id>                      # codex's own thread, continued by a resume
  output:  <path>                    # the final answer on disk, in this session's own directory
  events:  <path>                    # the raw event stream on disk

## 4. Iterate with the same run

IF the codex run needs iteration:
### Resume the same job
Use `codex-run resume <job> "<msg>"` with feedback so the run keeps full Context. It reruns under the founding Agent, on the same thread, off the job's own record. Do not re-explain to a fresh run.

IF a resume refuses to start:
### Start a fresh run instead of retrying
The three refusals are the job id naming no job or several, the founding Agent no longer being on the roster, and a run that never reached a codex thread. None becomes retryable. Correct the id, or dispatch a fresh `codex-run @<agent>` carrying the Context the resume would have kept.

IF the output says DID NOT RESUME:
### Judge the answer as a fresh run's
codex no longer held the thread, so the wrapper ran the turn on a new one from the job's own settings, with none of that thread's history. Feedback that referred back to earlier work reached an Agent that never saw it.

## 5. Close research gaps

### Re-dispatch thin work
Re-dispatch until every claim comes from a read. Stop only when no gap remains.

## 6. Verify the work

### Prove behavior against the repo
Check the repo or exercise the User-facing Critical Path.

IF a claim needs checking against what the run actually did:
### Fetch both Harnesses' records
`codex-run history <job>` names codex's rollout for the run's thread and this session's Claude transcript, so the run's own turn-by-turn record is readable next to your own.

### Do not edit while orchestrating
Every line of code is written by a codex run.

Never: Edit, Write, or NotebookEdit while orchestrating.

## 7. Synthesize yourself

### Use the /subagents ranking Process
Strip codex-run recommendations, keep facts, rank with /pcc, and present your own judgment.

Never: forwarding a run's ranking or recommendation.
