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
- Use LSP tools — go-to-definition, find-references over grep
- Fail fast — no defensive code. Crash loud. Validate at boundaries only
- Do the work normally — no clever bash scripts or optimization hacks. Go file by file
- Fix performance when problems appear, not before

<important if="you are refactoring, consolidating, or simplifying code">
- Refactoring means REDUCTION — fewer lines, fewer files, fewer abstractions
- If a refactoring task results in more code, it failed
- Never create new wrapper functions or add types "for clarity" — delete duplicate code and use existing patterns
</important>

<important if="you are adding error handling, try/catch blocks, or recovery logic">
- Development: Fail fast. Every error stops with clear message. No silent catches
- Production: Log everything. Non-critical features degrade gracefully
</important>

<important if="you are stuck, blocked, or considering a workaround">
- When hitting a wall: fix the design, not the symptoms
- Hacks accumulate. Architecture scales. Prefer the latter
- If a hack seems necessary: describe the architectural fix you're avoiding and why
- Temporary workarounds require: (1) architect approval, (2) documented rationale
- Say "I'm stuck because X. Should I Y or Z?"
</important>
