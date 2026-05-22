---
name: code-first
description: Autonomous async execution mode. The architect hands off, walks away, and the agent drives the task to done — makes every architectural call via /trace + /pcc + ranking, executes through /subagents (or /team for 3+ subtasks), validates by exercising the user-facing flow (real browser via the tester agent for UI), and returns finished code plus a decision log for review. TRIGGER when the architect says "code-first", "/code-first", "execute with /subagents", "execute directly & autonomously", "drive it and report back", "iterate until done", "I'm going to bed", "off to bed", or signals an AFK / overnight handoff. DO NOT TRIGGER when the architect wants options before action — that fires the default escalate-on-architecture mode (no skill loaded; the agent proposes via /pcc and waits).
---

# Code-First

The architect hands off and walks away; you drive to done. Make every
architectural call, execute through subagents, validate by exercising
the user-facing flow, return finished working code plus a decision log.
The architect owns architecture and exercises that ownership on running
code at review time, not on a proposal up front.

## Triggers

- "code-first", "/code-first"
- "execute with /subagents", "execute directly & autonomously"
- "drive it and report back", "iterate until done"
- "I'm going to bed", "off to bed", any AFK / overnight handoff

## What this mode replaces

The default doctrine has the architect approve each architectural call
before action. This skill replaces that one gate with post-hoc review,
because invoking it is the explicit handoff. Outside this skill, the
default holds.

## Baseline

Establish the entry baseline in writing before anything else:

- Run the test suite. The result is the contract; final state matches
  or exceeds it.
- Read the relevant area enough to know what currently works for the
  user. Final state preserves every capability.

The architect's framing — "the code works right now, the tests work,
that's what we expect at the end" — is the contract. A broken-before
deflection by any subagent is checked against this baseline.

## Validation SOP

Build the validation SOP from the code, not from your head. Dispatch
a research subagent (/trace + /subagents) to study the area, map the
user-facing flows the task touches, and return a concrete plan: every
flow, every input, every expected output.

That plan is the validation contract. Keep it fresh in context with
/loop so it doesn't drop out as the work expands.

Validation is exercised behavior — see /subagents. UI flows run in a
real browser through the tester agent. Compile, type check, and
confidence number are never validation (compile-as-validation,
confidence-as-validation).

## Architectural calls

Every architectural call follows the same workflow. You make the
call. The architect reviews the recorded calls at the end.

1. /trace and explore the code first. Trace before any urge to ask.
2. /pcc — surface real options with pros, cons, confidence.
3. Rank against the litmus: user, architecture, business. Never
   effort, file count, or speed.
4. Pick the best one.
5. Record the call in the decision log.

## The decision log

Return one artifact at the end. Architectural calls only — never a
process recap. Order them so context compounds: a call that
establishes a fact comes before calls that rely on it.

For each call:

- **Decision** — one sentence: what you chose.
- **Trigger** — what surfaced this as a decision (a failing test, a
  missing route, a contract gap, a user-reported outcome).
- **Why** — the user / architecture / business reason it serves.
- **Alternatives** — what you weighed and rejected, one line each.
- **Touched** — modules, contracts, or data, in architect-voice.
- **Validation** — exact command and exact output. Browser evidence
  for UI flows (tester agent).
- **Confidence** — %, and the remaining risk. A number below certain
  means a path is untested — exercise it before logging the call.

Calls are independent. The architect accepts or rejects each one. A
rejection re-dispatches that slice with the correction; accepted
calls stand.

Example entry:

- **Decision** — Forms endpoint exposed as
  /api/v1/builder/form-submit; one controller handles both variants.
- **Trigger** — Live dent.js posts to a /wp-json/bricks/v1/form/
  route that doesn't exist in Bricks 2.x; native form pixel silently
  dead.
- **Why** — Architecture goal: a single trustworthy entry through
  Laravel middleware. User benefit: form tracking that doesn't depend
  on plugin internals.
- **Alternatives** — (a) Hook into Bricks' admin-ajax handler
  in-place — keeps the WP hooks hell. (b) Two controllers per variant
  — preserves old labeling but variants are collapsing.
- **Touched** — New controller pair routes; api middleware group now
  runs for builder form posts.
- **Validation** — POSTed `{email: a@b.co}` via the live form; got
  200 with redirect to /thanks; optin pixel fired in the network
  panel (tester agent, real browser).
- **Confidence** — 95%. Remaining risk: third-party form-builder
  variants we don't currently render.

## Blockers

Most claimed blockers are blocked excuses — see /subagents. A real
blocker survives the retry / restart / rebuild ladder and every
honest path crosses a boundary the architect must rule on.

When a real blocker hits:

- Log it in the decision log under "Pending architect input": the
  item, the paths attempted, what each crosses.
- Pause that one item. Other work continues.
- The whole run stops only when shared state is corrupted and needs
  a manual restore (see /subagents).

## Background work cadence

Long-running processes — test runs, builds, async subagents — get
checked on a 30-minute wall-clock cadence. Fast polling is the
**context-drain** failure mode: each check eats the context needed
for synthesis at the end. The harness re-invokes you when tracked
work finishes.

## Hard Rules

- **You make the calls.** Reverting to "which option do you want?"
  is the default mode, not this one.
- **You verify, hold the goal, and orchestrate; you do not
  implement.** /trace small files for direct checks; anything larger
  is a hard gate handled by a verification subagent.
- **Validation is exercised behavior.** UI flows through the tester
  agent in a real browser. Compile-as-validation and
  confidence-as-validation are banned.
- **Issues are 99% subagent laziness, 1% everything else.** Treat
  them so. Hold subagents accountable; prove every claim.
- **The architect is right about reported outcomes.** Shallow
  reframes get re-dispatched, not believed.
- **Destructive-git restore is banned.** The agent that broke a file
  may restore it manually; never a script, never a blind nuke.
- **Architecture stays the architect's to own.** Code-first means
  reviewed after, not unowned.

## Process

Run top to bottom without pause:

1. **Baseline** — run the test suite, read the relevant area, record
   the entry baseline.
2. **Validation SOP** — dispatch a research subagent to map the user
   flows and return a validation plan. Keep it fresh via /loop.
3. **Architectural calls** — for each: /trace, /pcc, rank, pick,
   record (with trigger).
4. **Execute** — orchestrate via /subagents (or /team for 3+). Apply
   the full no-bullshit doctrine. Take the long hard way.
5. **Validate** — run the SOP every iteration. UI flows in a real
   browser via the tester agent. Baseline tests stay green.
6. **Iterate until done** — every DoD met, every SOP item green, no
   coping accepted. 99% of issues live in subagent work.
7. **Return** — finished code plus the decision log. Pending blockers
   surfaced separately. Stop.
