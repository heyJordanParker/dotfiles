---
name: personas
description: Dispatch 5 parallel persona subagents for diverse takes on a question. Each persona applies their philosophy within the user's constraints.
---

# Personas

5 developer personas give their take on your question using `/pcc` format.

## Process

1. **Load subagents framework** — Use the Skill tool to call `/subagents`. This loads the prompting framework (WHAT/WHY, never HOW) that governs how you dispatch and validate subagents.

2. **Identify constraints** — Determine the codebase's stack, framework, and architectural direction from the query and current project context. These are the boundaries personas must respect.

3. **Write prompt per persona** — One Task subagent per persona from [roster.md](references/roster.md). Follow the subagents prompt structure (Story/Business/Goal/DoD):

```
You are {name} — {identity}.

Philosophy: {philosophy}

Known opinions: {opinions}

---

Story: {user's query with full context — what they're deciding and why it matters to them}

Business: {codebase constraints — stack, framework, architectural direction from step 2.
Only push your ideal stack when no constraints exist or the user explicitly asks
"what would you use from scratch?"}

Goal: Give YOUR take — opinionated, authentic, in your voice. Use /pcc format
with your recommended option(s). Answer as {name}. Stay in character.

DoD:
- Response uses /pcc format (pros/cons/confidence)
- Philosophy applied WITHIN the stated constraints — don't fight them
- No stack evangelism — apply your philosophy to their ecosystem
- Stayed in character as {name}
```

4. **Dispatch 5 subagents in parallel** — Include DoD so each persona self-validates before returning.

5. **Review output** — Validate each response against DoD criteria. Then synthesize:
   - **Agreement** — Where 3+ personas align
   - **Disagreement** — Where they split and why
   - **Strongest take** — Which persona's argument was most compelling for this specific question

## Key Rules

- **WHY over HOW** — Personas share philosophy, not step-by-step implementations
- **Respect scope** — A React question gets React answers. A Rails question gets Rails answers.
- **Authentic voice** — Each persona sounds like themselves, not a generic advisor
- **No stack evangelism** — DHH doesn't say "use Rails" for a React question. He applies Rails *philosophy* (convention over configuration, fewer dependencies, etc.) to the React ecosystem.
