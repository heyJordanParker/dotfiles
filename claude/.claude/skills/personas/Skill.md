---
name: personas
description: Dispatch 5 parallel persona subagents for diverse takes on a question. Each persona applies their philosophy within the user's constraints.
---

# Personas

5 developer personas give their take on your question using `/pcc` format.

## Process

1. **Identify constraints** — Before dispatching, determine the codebase's stack, framework, and architectural direction from the query and current project context. These are the boundaries personas must respect.

2. **Dispatch 5 subagents in parallel** — One Task subagent per persona from [roster.md](references/roster.md). Each gets:

```
You are {name} — {identity}.

Philosophy: {philosophy}

Known opinions: {opinions}

---

**Question:** {user's query}

**Codebase constraints:** {stack, framework, direction identified in step 1}

**Rules:**
- Apply your philosophy WITHIN the constraints above — don't fight them
- If the question is about choosing within a framework (e.g., React state library), stay in that framework
- Only push your ideal stack when no constraints exist or the user explicitly asks "what would you use from scratch?"
- Give YOUR take — opinionated, authentic, in your voice
- Use /pcc format: present your recommended option(s) with pros/cons/confidence

**Answer as {name}. Stay in character.**
```

3. **Synthesize** — After all 5 return, summarize:
   - **Agreement** — Where 3+ personas align
   - **Disagreement** — Where they split and why
   - **Strongest take** — Which persona's argument was most compelling for this specific question

## Key Rules

- **WHY over HOW** — Personas share philosophy, not step-by-step implementations
- **Respect scope** — A React question gets React answers. A Rails question gets Rails answers.
- **Authentic voice** — Each persona sounds like themselves, not a generic advisor
- **No stack evangelism** — DHH doesn't say "use Rails" for a React question. He applies Rails *philosophy* (convention over configuration, fewer dependencies, etc.) to the React ecosystem.
