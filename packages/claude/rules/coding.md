---
paths: **/*.{ts,tsx,mts,cts,js,jsx,mjs,cjs,php,py,go,rs,rb,java,c,cpp,h,hpp,cs,swift,kt,sh,bash,sql,css,scss,sass,html,json,yml,yaml,toml,xml,env}
---

# Coding

For every code file in this session:
- Read it fully before discussing or changing
- Describe what it does before modifying
- List existing behavior that must be preserved

Before any code assertion (what code does, returns, calls, contains, causes):
- Did you read the source?
- Did you trace the actual code path?
- Pattern matching is not validation — read it or say "I haven't checked"

Before claiming code works:
- Did you run tests or show evidence?

Before re-verifying:
- The baseline works until evidence shows otherwise
- Verify what you changed and what your change reaches — do not re-verify unchanged code with no reason to be broken

When creating new files:
- Could this go in an existing file instead?
- Is this abstraction needed now, or "might be useful later"?
- Use /naming skill for all file, class, variable, and function names

## Principles

- Simplest solution that works
- One responsibility per file, strict encapsulation
- Prefer libraries and services over building ourselves
- Small, decoupled pieces that can be swapped or rewritten
- Read docs first — understand before using
- No hedging — "I don't know" beats "might/should/probably"
- Use the trace skill for code intelligence over raw grep
- Fail fast — no defensive code. Crash loud. Validate at boundaries only
- Do the work normally — no clever bash scripts or optimization hacks. Go file by file
- Fix performance when problems appear, not before

<important if="you are refactoring, consolidating, or simplifying code">
- Refactoring means REDUCTION — fewer lines, fewer files, fewer abstractions
- If a refactoring task results in more code, it failed
- Never create new wrapper functions or add types "for clarity" — delete duplicate code and use existing patterns
</important>

<important if="you hit code that looks unused, dead, half-wired, or unfinished">
- "Looks unused" is never "is unused." Usage hides in routes, config, dependency injection, reflection, serialization, external callers, agents, and half-built features. You cannot prove code is dead by reading it, and counting references proves nothing — never quantify usage to justify the change
- Assume a competent author wrote it to solve a real problem under context you may not have. Start from "why would someone write this?", never "this is junk" — assuming prior work was stupid is how you delete something load-bearing
- Recover the WHY: read the code, what it touches, and its git history and blame. State in one sentence the problem it was written to solve. Can't? You don't understand it well enough to touch it — keep digging or leave it
- Determine its real status by reasoning, not measuring: live but reached in a way static reading misses, staged for a feature still being built, or genuinely orphaned because its problem is gone
- Propose a course of action that still solves that same problem the best way for the situation now; lay out several with /pcc if they exist. Removal is valid only once you can name the problem and show it no longer exists
- Deleting or rewriting code whose purpose you cannot state is the failure. Failure modes: looks-unused-equals-unused, stupid-predecessor, silent delete/rewrite
</important>

<important if="you are adding error handling, try/catch blocks, or recovery logic">
- Development: Fail fast. Every error stops with clear message. No silent catches
- Production: Log everything. Non-critical features degrade gracefully
</important>

<important if="you are stuck, blocked, or considering a workaround">
- When hitting a wall: fix the design, not the symptoms
- Hacks accumulate. Architecture scales. Prefer the latter
- If a hack seems necessary: describe the architectural fix you're avoiding and why
- Temporary workarounds require: (1) architect approval, (2) documented reasoning
- Say "I'm stuck because X. Should I Y or Z?"
</important>
