---
name: subagents
description: Framework for dispatching one-shot subagents that complete a task and return. Covers prompting (WHAT/WHY, never HOW), prompt structure (Story/Business/Goal/DoD/Workflow), and validation via DoD. For persistent teams, use /team.
---

# Subagents

Framework for dispatching one-shot subagents — agents that complete a single task and return results. For persistent teams that coordinate across multiple tasks and slices, use the /team skill.

## Triggers

- Dispatching any subagent (implementation, research, review)
- Running parallel independent agents
- One-shot validation or analysis tasks

## Prompting

### Tell agents WHAT and WHY. Never HOW.

Agents have fresh context. Detailed implementation instructions bias them toward your assumptions instead of letting them find the right solution.

When you know HOW to solve something, you instinctively dump that into the prompt. This:
- Locks the agent into your approach (which may be wrong)
- Wastes tokens on instructions they'd figure out by reading code
- Prevents them from finding a better solution
- Creates fragile prompts that break when code changes

**Exception:** Mechanical tasks (bulk renames, format conversions) are fine with specific instructions since they're not architectural.

### Scope agents to their reasoning unit, not your diff.

An architect reviewing one method can't assess encapsulation. A code reviewer scoped to one hunk can't find regressions. Give review and architecture agents the full module or feature — they narrow themselves after reading the code.

- Architect → entire module/feature, not the changed file
- Code reviewer → full diff + surrounding context, not individual hunks
- Backend engineer → the service boundary, not the changed endpoint

### Good vs Bad Prompts

**Bad: Step-by-step instructions**

```
Fix the payment timeout bug:

1. Open PaymentService.php
2. Find the processPayment method on line 142
3. Add a try-catch around the Stripe API call
4. In the catch block, check if it's a timeout exception
5. If timeout, retry up to 3 times with exponential backoff
6. After retries exhausted, throw PaymentTimeoutException
7. In PaymentController.php, catch PaymentTimeoutException
8. Return a 408 response with message "Payment timed out"
9. Write a test that mocks Stripe to throw timeout
10. Verify retry behavior
```

Problems: Assumes the solution. Dictates file structure. Specifies implementation details the agent should discover.

**Good: WHAT and WHY**

```
Story: Users on slow connections see checkout spin forever, then
nothing. No error message, no retry, order stuck in "pending."

Business: 12% of failed checkouts are timeout-related. Retry logic
exists in Stripe SDK but we're not surfacing its results to the UI.

Goal: Surface timeout errors to UI and pass through Stripe retry
results.

DoD:
- Timeout shows user-friendly error message
- Stripe retry success completes the order
- Tests cover timeout and retry paths

backend/
├── Controllers/PaymentController.php   <- handles checkout endpoint
├── Services/PaymentService.php*        <- timeout logic lives here
├── Models/Order.php                    <- order status tracking
└── tests/
    └── PaymentServiceTest.php*         <- add timeout test coverage
```

**Bad: Pre-researched paths and prescribed investigation order**

```
Build a new top-level `trace blame` command.

The command lives in `src/tracer/commands/blame.py` and registers on the
click group in `src/tracer/__main__.py`. Read `src/tracer/commands/read.py`
first to see the `_extract_method` pattern, then `src/tracer/file_facts.py`
for FileFacts, then `src/tracer/passive_context.py` for the shoulder.

Use `lizard.analyze_file.analyze_source_code(...)` to resolve the symbol's
line range. Shell out to `git blame --line-porcelain` for raw blame, then
parse the porcelain envelope. DO NOT modify `src/tracer/__main__.py` —
registration is handled centrally.

DoD: ...
```

Problems: looks like WHAT/WHY at first glance, but every concrete file
path, function name, and ordering instruction is a HOW decision smuggled
in as context. Specific triggers that get this rejected:

- file paths (`src/tracer/commands/blame.py`)
- function/symbol names (`_extract_method`, `FileFacts`)
- ordering instructions (`Read X first, then Y`)
- prescribed library calls (`use lizard.analyze_file.analyze_source_code`)
- mechanical guardrails (`DO NOT modify __main__.py`)

The agent has fresh context; it will find the read pattern, the file
facts module, and the shoulder by exploring the codebase from the Goal.
The list above does that work for it, biases the implementation, and
encodes assumptions you may have gotten wrong.

**Good: Same intent, agent left to discover the codebase**

```
Story: Empirical transcript analysis found `git blame -L <range>:<file>`
is used zero times despite constant ownership questions of the form
"who last touched function X" — agents currently simulate this by
reading `git log` output and guessing which commit modified which
region.

Business: gives the agent a clean, scoped ownership answer for any
region of any file, instead of the line-by-line porcelain dump or the
manual log-and-guess workaround.

Goal: build a new top-level `trace blame` command in the tracer
codebase. The command returns blame information for a file, scoped to
either the whole file, a specific line range, or a named symbol.
Symbol scoping is the novel capability — agents can ask "who owns this
function" without thinking in line numbers.

DoD:
- `trace blame <file>` blames the whole file
- `trace blame <file> <symbol>` resolves the symbol and blames its range
- `trace blame <file> --lines L1:L2` blames an explicit range
- Each result region carries the commit subject inline
- Consecutive identical-commit lines collapse to a single region

Constraints:
- Tracer's command structure, output shape, and dependency handling
  follow established conventions — discover and match them.
- No new external binary dependencies.
- Command registration is centrally managed; you are responsible only
  for the new command module.
```

The Story explains the user pain, the Goal names the capability and the
novel bit, the DoD is observable. Nothing names a file, function, or
library — the agent finds those by exploring from the Goal.

**Bad: Vague one-liner**

```
Fix the payment bug.
```

Problems: No context. No way to validate. No scope boundaries.

**Bad: Over-scoped to the diff**

```
Review the `calculateDiscount` method in PricingService.php.
Check if the new early-return is correct.
```

Problems: Architect can't assess encapsulation or dependency direction from one method. Reviewer can't find regressions in callers.

**Good: Scoped to the reasoning unit**

```
Review PricingService — we changed the discount calculation logic.
Apply your full review protocol across the service.
```

The agent narrows itself once it reads the code. Your job is to give it enough room to find things you didn't think to look for.

### Weaving Context

Previous findings go into the section where they belong — not a separate "gotchas" or "notes" section.

**Bad:** Dumping findings into a "Notes" or "Gotchas" section at the end.

**Good:** JWT bug goes in Story (user impact). CORS ordering goes in Business (debugging constraint). The agent gets context where it matters.

### DoD Guidelines

DoD is how the agent validates its own work before returning. Make it:

- **Observable** — can be verified by running something or checking output
- **Specific** — "tests pass" not "code works"
- **Complete** — covers the actual goal, not just the happy path

**Bad DoD**

```
DoD:
- Code works
- Tests added
- No errors
```

**Good DoD**

```
DoD:
- `npm test -- --grep "payment"` passes
- Timeout after 30s shows "Payment timed out. Please try again."
- Successful retry completes order (status changes to "paid")
- Failed retry after 3 attempts shows "Unable to process payment"
```

## Your job: verify, hold the goal, orchestrate

You do three things and nothing else: verify the work, keep the goal
above all else, orchestrate the agents. Implementation is for
subagents; massive-changeset reading is for verification subagents.

- /trace or read directly for verifying a single claim against <200
  LOC total.
- Anything larger is a hard gate — dispatch a verification subagent.
  Reading a wide diff yourself is the **context-burn** failure mode:
  you lose the context you need for orchestration and miss things in
  the diff.

## You do the ranking. Subagents do not.

A subagent sees a slice. You see the project — its conventions, the
architect's prior calls, sibling code. When a subagent returns
"Option 1 is best", that ranking was made without any of it. Pasting
it through is how the architect ends up reading a confident-sounding
call made by an agent who could not see the code that should have
ruled the call out.

So a subagent's recommendation is one of its findings. Strip the
recommendation. Keep the facts. Then do the ranking yourself:

1. **Run the research until it is done.** The first batch back is
   not the research; it is the first batch. Dispatch every
   independent gap in parallel and re-dispatch the agents whose
   work came back thin. Stop when no gap is left, not when the
   first batch returns. Every claim under every surviving option
   must come from a read, not a guess.
2. **Eliminate.** Drop every option that breaks a standard,
   convention, or project rule. You do this — not a subagent.
3. **Rank the survivors with /pcc.** Your /pcc. Your pros, cons,
   confidence — informed by everything a subagent could not see.
4. **Recommend one.** From your ranking. In your voice.

Banned phrases:
- "the agent recommends"
- "per the research"
- "based on the findings, X is best"
- "Architect recommended Option N"
- "following the analysis, Option N"
- anything that hangs the recommendation on the subagent's authority

Research-subagent prompts close with this paragraph, verbatim:

> Close every research gap. Dispatch as much in parallel as is
> independent. Do not stop to ask, do not deliver half-finished
> work, run the loop until nothing is left to investigate. Every
> code claim must come from a read, not a guess. Findings only —
> no scope changes, no ranking, no recommendation.

Default overridden: forwarding the subagent's recommendation as
the dispatcher's call, and treating the first batch of findings
as the research.

## Subagent output is a claim until you prove it

A subagent's summary describes what it believes it did. Three things
outrank it: the repo, common sense grounded in the user / architecture
/ business, and the architect's reported outcome.

Each pillar below names the failure mode the agent should self-label
with when it catches itself producing it.

- **Shallow reframe.** The architect's reported outcome outranks any
  subagent finding. When a subagent's research contradicts what the
  architect reported, the subagent is incomplete — not the architect.
  Banned phrases: "you probably meant X from weeks ago", "that's
  already fixed", "you didn't see it correctly". The default
  explanation for any discrepancy is incomplete research; re-dispatch
  deeper until a subagent reproduces the reported outcome.
- **Unproven claim.** Prove every load-bearing claim against the repo
  before accepting it. "Done", "tests pass", "already correct", "no
  change needed" earn belief only after a repo check — by you for
  <200 LOC, by a verification subagent for anything larger.
- **Scope cop-out.** Scope is set by the user, the architecture, and
  the business. "Out of scope", "too many files", "too slow", "too
  much" are effort arguments, not scope arguments. If the work serves
  any of the three litmus tests, it is in scope however large.
  Re-dispatch with the scope restated.
- **Broken-before deflection.** "It was broken before" is false until
  proven by a clean baseline run. The repo does not sit perpetually
  broken; ~99% of the time the agent's own change broke what's broken.
- **Blocked excuse.** "I'm blocked" is usually a skipped simple step.
  Retry, restart the dev server, re-run the command, reinstall, clear
  the cache. Assume the claim is wrong until a second agent reproduces
  it.
- **Coping.** "Mostly works", "blocked so I did Y instead", "couldn't
  run tests but the logic is correct" is not completion. Send it back
  with the specific gap.
- **Polluted context.** When an agent produces clearly wrong work,
  fresh context beats arguing it into correctness. Re-dispatch — to
  it or a new agent.
- **Forwarded recommendation.** Pasting a subagent's ranking, option
  choice, or "best option" through as your own. The subagent saw a
  slice; you hold the whole. Strip the recommendation. Re-rank with
  your own /pcc, in your own voice. Banned phrases: "the agent
  recommends", "per the research", "based on the findings, X is
  best", "Architect recommended Option N", "following the analysis,
  Option N" — anything that hangs the recommendation on the subagent.
- **First-batch-as-final.** Treating the first batch of findings as
  the research. The first batch is the first batch. Re-dispatch
  every gap in parallel, re-run the agents whose work came back
  thin, keep going until every claim is grounded in a read. Stop
  when there is nothing left to investigate, never when the batch
  returns.

Example — shallow reframe vs correct response:
- Bad: architect reports "form pixel isn't firing"; subagent finds a
  fix from 2 weeks ago and concludes "that's already fixed, you
  probably mean the old bug". Skill output: "the issue is already
  fixed."
- Good: architect reports "form pixel isn't firing"; subagent finds a
  prior fix that doesn't reproduce the symptom; re-dispatch with the
  reported outcome restated and the prior fix as a non-explanation.
  Skill output: "re-dispatching; prior fix doesn't reproduce the
  reported symptom."
- Why: the architect saw the outcome. A finding that doesn't
  reproduce it is incomplete research, not a correction to the
  architect.

## Validation is proof it works, not a guess

Validation is the user-facing capability exercised end to end with a
concrete input and the observed output shown. State every criterion
as input → output, a command + expected status and body, or an
endpoint + expected result.

Failure modes the agent self-labels with:

- **Compile-as-validation.** Compiling, type-checking, linting, "the
  logic is sound", "looks correct" prove nothing about behavior. They
  are necessary, never sufficient. Run the user flow.
- **Confidence-as-validation.** A percentage instead of a test is a
  guess wearing a number. "~30%, wouldn't stake my life on it" means
  the path was never exercised.
- **Guess-as-test.** "Should work", "observably correct", "looks
  fine" are not validation. Either an exact input and output, or it
  did not run.

Before accepting any "done", the agent answers in writing:

- What concrete input did I run, and what exact output came back?
- Which user-facing flow did I exercise, start to finish?
- What did I NOT exercise, and what breaks if that path is hit in
  production?
- Would I stake my life on this running in production?

The life-stake question is the forcing function. A number below
certain names an untested path. The fix is never to report the low
number — exercise the path until the number is real, then report the
input and output. An agent returning a low confidence instead of the
missing test has not finished.

Example — compile-as-validation vs correct response:
- Bad: "validated — `npm run build` succeeds, types check, confidence
  80%."
- Good: "validated — POSTed `{email: a@b.co}` to /api/v1/builder/
  form-submit; got 200 with redirect to /thanks; optin pixel fired
  (network tab, tester agent). 100% on the happy path; haven't
  exercised the Bricksforge variant."
- Why: behavior, not toolchain output, is the proof.

## Prompt Structure

Every agent dispatch uses these sections:

- **Story** — What the user experiences and needs
- **Business** — Why this matters, constraints, limitations
- **Goal** — What the agent delivers, expected output
- **DoD** — How the agent validates its own work
- **Workflow** — Task state transitions that frame the work (see below)

### Workflow Section

Every prompt ends with a Workflow section. This is the agent's operating procedure — not a footnote. It's the first thing the agent does and the last thing the agent does, sandwiching all implementation work.

```
Workflow:
1. Read every file marked * in the architecture block above
2. One file at a time — read each file, then edit it. No bulk-rewrite scripts, no clever shortcuts (cute shortcut)
3. Implement against the Goal
4. For EACH DoD item: run verification, paste relevant output
5. If any DoD item fails → fix and re-verify (loop step 4)
6. Post a completion summary: what changed, what was verified, what was tricky
```

### Architecture Block

Before the Workflow section, include an annotated file tree + 1 paragraph of context:

```
Payment timeout errors silently swallow failures. The payment service
uses BaseService patterns. Controllers return WP_REST_Response objects.

backend/
├── Controllers/PaymentController.php   <- handles checkout endpoint
├── Services/PaymentService.php*        <- timeout logic lives here
├── Models/Order.php                    <- order status tracking
└── tests/
    └── PaymentServiceTest.php*         <- add timeout test coverage
```

## Hard Rules

- **You do NOT use Edit, Write, or NotebookEdit** when coordinating multiple agents. Every line of code is written by a subagent. You preserve your context window for coordination, not implementation
- **One-shot agents return and die.** They don't persist. For work that needs iteration, feedback loops, or multi-slice coordination, use the /team skill instead
- **Destructive-git restore is banned.** Restore by hand only. Never `git reset`, `git restore`, `git checkout --`, never a script, never a blind nuke. The agent that broke a file may restore it to its previous state manually; the architect may do it. The hooks enforce this because subagents nuke without status-checking and destroy real work.
- **Take the long hard way.** Editing 100 files is 100 reads and 100 edits, file by file, spread across agents if large. Read each file, then edit it, in the simplest possible process. Failure mode: **cute shortcut** — a script that bulk-rewrites, or a clever one-liner that skirts the work and hides errors.
- **Never forward a subagent's ranking or recommendation.** Subagents return facts. You eliminate bad options, you finish the research, you rank with /pcc, you recommend one — all in your own voice.

## Dispatching

### Parallel (independent tasks)

Spawn all agents at once using the Agent tool. Each works independently with `run_in_background: true`.

```
Agent(
  subagent_type: "backend-engineer",
  name: "worker-name",
  prompt: "Story, Business, Goal, DoD + Architecture + Workflow",
  run_in_background: true
)
```

## Quick Reference

- **Need persistent workers?** → Use /team skill
- **Need non-blocking work?** → `run_in_background: true`
- **Want to give step-by-step?** → Stop. Give WHAT/WHY instead
- **Scoping to one method?** → Stop. Give the full module/feature
- **Agent reports a confidence number instead of a test?** → confidence-as-validation. Exercise the path; re-dispatch
- **"It compiles / type-checks / looks correct"?** → compile-as-validation. Run the user flow
- **Big changeset to verify?** → Hard gate. Dispatch a verifier; reading it yourself is context-burn
- **Subagent contradicts what the architect reported?** → shallow reframe. Re-dispatch deeper
- **Agent says "done"?** → unproven claim until proven against the repo
- **Agent says "out of scope" / "too many files"?** → scope cop-out. Effort, not scope. Re-dispatch
- **Agent says "broken before"?** → broken-before deflection. False until a clean baseline run proves it
- **State corrupted?** → Manual restore only. Destructive-git restore is banned
- **Agent returned a ranking or "best option"?** → forwarded recommendation. Strip it. Re-rank yourself with /pcc
- **First batch of findings back?** → first-batch-as-final. Loop until every gap closes. Re-dispatch the thin agents
- **About to write "the agent recommends" or "per the research"?** → stop. That sentence is the failure

## Process

1. **Assess** — is this one-shot work or does it need iteration? One-shot → here. Iteration → /team
2. **Write prompts** — Story, Business, Goal, DoD, Workflow
3. **Add architecture** — annotated file tree before Workflow section
4. **Dispatch** — specialized `subagent_type`, every prompt includes Workflow as final block
5. **Review output** — against DoD criteria
6. **Verify, hold the goal, orchestrate** — prove every returned claim by exercising the user-facing flow against the repo, the user/architecture/business litmus, and the architect's reported outcome. Large changesets go to a verification subagent.
