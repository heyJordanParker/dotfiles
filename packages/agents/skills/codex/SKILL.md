---
name: codex
description: Drive codex CLI runs as your agents — you stay the orchestrator, codex does the work. One wrapper, `codex-run`, owns the mechanics (flags, output storage, stream parsing, failure detection); you write the task prompt and do the judgment. Mirrors /subagents doctrine (hold the goal, prompt WHAT/WHY never HOW, dispatch in parallel, verify every claim, rank yourself). TRIGGER when the architect says "codex", "/codex", "codex-run", "use codex", "dispatch to codex", "run this through codex", "codex agents", "review with codex", "codex review", or asks to fan out work across codex runs. DO NOT TRIGGER for native Claude Code subagents (use /subagents) or persistent Claude teams (use /team) — those run inside this harness; codex is a separate CLI process.
---

# codex

Drive codex runs as your agents. You hold the goal and orchestrate; each codex run is one agent that does the work and returns. This is `/subagents` doctrine with codex as the execution substrate — every orchestration rule there applies here.

One command owns every mechanical part of a run — `codex-run`. You never hand-assemble a `codex exec` invocation, never parse the event stream, never grep for failure. You write the prompt and do the judgment; the wrapper does the rest.

## The interface: `codex-run`

```bash
codex-run @<agent> "<prompt>"        # run codex as <agent>, return its final answer
codex-run resume <session> "<msg>"   # continue a prior run with full context
```

`@<agent>` is one of our named agents — it resolves to that agent's own instructions and runs codex under them (the same mechanism that boots codex as the CTO). An unknown agent exits non-zero and lists the available ones, so a typo is self-correcting — read the list and pick.

The wrapper is the whole interface. It owns every mechanical step — flags, the no-sandbox run, all event-stream parsing, output storage, failure detection — and prints one clean result to **stdout**, ready to read as-is. You never parse its output, never pipe it through anything, never touch the event stream by hand.

The result is the run's **final answer**, then a delimited trailer:

```
<the final answer codex produced>
--- codex-run ---
status:  ok            # or "failed"
session: <id>          # the run's identity — pass to `codex-run resume`
output:  <path>        # the final answer on disk, in this session's own directory
events:  <path>        # the raw event stream on disk, if you ever want to inspect the run
```

Read the answer above the `--- codex-run ---` line; read the session id, status, and paths below it. Everything you need is on stdout — there is nothing on stderr to collect and no post-processing to run.

Parallel runs get collision-free output paths automatically — background several and they never clobber each other.

**The wrapper exits non-zero on failure** — a process failure or a failed turn (even one that lies "done" in its prose). A zero exit means the run completed its turn; the answer is still a claim you must prove (below).

### Mandatory: every codex-run goes in the background

A foreground `codex-run` blocks your orchestration on the codex turn. Run every one with `run_in_background: true` on the Bash call — a guard blocks it otherwise. Fan out by launching several backgrounded runs and reading each back when it lands.

### Review is just an agent run

```bash
codex-run @code-reviewer "review the uncommitted changes"
```

There is no review mode and no embedded review prompt. The review lens lives in the code-reviewer agent's own instructions; routing review through `@code-reviewer` is how you get it. Scope the prompt to what to review (the diff, a feature, a module) — the lens is already loaded.

## The prompts you send — WHAT/WHY, never HOW

A codex run has fresh context, like any subagent. Detailed implementation steps bias it toward your assumptions instead of letting it find the right solution. Every prompt uses the `/subagents` structure:

- **Story** — what the user experiences and needs
- **Business** — why it matters, constraints, limitations
- **Goal** — what the run delivers, the observable output
- **DoD** — how the run validates its own work (observable, specific, complete)
- **Workflow** — read the marked files, implement against the Goal, verify each DoD item with pasted output, post a completion summary

**Tell the run WHAT and WHY. Never HOW.** No file paths, no function names, no "read X then Y", no prescribed library calls. The run discovers the codebase from the Goal. (Exception: mechanical tasks — bulk renames, format conversions — take specific instructions; they aren't architectural.) Naming *what* to review or change — specific files, a diff, a module — is scope, and scope is allowed; it's the implementation HOW (which steps, which calls, in which order) that the rule forbids.

Scope each run to its reasoning unit, not your diff: an architect run gets the whole module, a reviewer run gets the full diff plus surrounding context.

## codex output is a claim until you prove it

A run's final answer describes what it believes it did. It is not proof. Three things outrank it: the repo, common sense grounded in the user/architecture/business, and the architect's reported outcome.

- **Unproven claim.** "Done", "tests pass", "already correct" earn belief only after a repo check — by you for <200 LOC, by a verification codex run for anything larger. Reading a wide diff yourself is **context-burn** — you lose the context you need to orchestrate.
- **Validation means it ran.** Exercise the user-facing flow end to end with a concrete input and show the observed output. Compiling, type-checking, "looks correct", and a confidence number are not validation.
- **Scope cop-out.** "Out of scope", "too many files", "too slow" are effort arguments, not scope arguments. If the work serves the user/architecture/business, it is in scope however large — re-dispatch with the scope restated.
- **Shallow reframe.** When a run's output contradicts what the architect reported, the run is incomplete — not the architect. Re-dispatch until a run reproduces the reported outcome.
- **Polluted context.** When a run produces clearly wrong work, a fresh run beats arguing it into correctness — spawn a new run rather than resuming the confused one.

Before accepting any "done": what concrete input did the run exercise, what exact output came back, which user-facing flow ran start to finish, and would you stake the result on it running in production?

## You do the ranking. codex runs do not.

A codex run sees a slice. You see the project — its conventions, the architect's prior calls, sibling code. When a run returns "Option 1 is best", that ranking was made without any of it. Forwarding it through is how the architect ends up reading a confident-sounding call made by an agent who couldn't see the code that should have ruled it out.

A run's recommendation is one of its findings. Strip the recommendation, keep the facts, then do the ranking yourself:

1. **Run the research until it is done.** The first batch back is the first batch, not the research. Re-dispatch every gap in parallel and re-run the runs whose work came back thin. Every claim under every surviving option comes from a read, not a guess.
2. **Eliminate.** Drop every option that breaks a standard, convention, or project rule. You do this — not a run.
3. **Rank the survivors with /pcc.** Your /pcc, informed by everything a run could not see.
4. **Recommend one.** From your ranking. In your voice.

Banned phrases: "the codex run recommends", "per the run", "based on the findings, X is best", "following the analysis, Option N" — anything that hangs the recommendation on the run's authority.

Research-run prompts close with this paragraph, verbatim:

> Close every research gap. Do not stop to ask, do not deliver half-finished work, run until nothing is left to investigate. Every code claim must come from a read, not a guess. Findings only — no scope changes, no ranking, no recommendation.

## Hard rules

- **You do NOT use Edit, Write, or NotebookEdit when orchestrating codex runs.** Every line of code is written by a codex run. You preserve your context for orchestration.
- **A codex run returns and dies.** For work that needs iteration, `codex-run resume <session>` with feedback — it keeps full context — rather than re-explaining to a fresh run.
- **Take the long hard way.** No bulk-rewrite scripts in a run's prompt; read then edit, file by file. The **cute shortcut** (a script that bulk-rewrites and hides errors) is the failure mode.
- **Never forward a run's ranking or recommendation.** Runs return facts. You eliminate bad options, finish the research, rank with /pcc, recommend one — in your own voice.

## Process

1. **Assess** — independent work for one or more codex runs? Yes → here.
2. **Write prompts** — Story, Business, Goal, DoD, Workflow. WHAT/WHY, never HOW.
3. **Dispatch** — `codex-run @<agent> "<prompt>"`, backgrounded. Parallel independent work via several backgrounded runs.
4. **Read back** — final answer on stdout, `session:` for resume, non-zero exit for failure.
5. **Verify, hold the goal, orchestrate** — prove every returned claim by exercising the user-facing flow. Large changesets go to a verification codex run. Resume a run for iteration; spawn fresh when context is polluted.
6. **Rank yourself** — strip every run's recommendation, eliminate, rank with /pcc, recommend one in your voice.
