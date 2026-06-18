---
name: debate
description: Run N independent architects debating architectural options through structured rounds. Use when the user wants multiple competing solutions evaluated, or when 3+ distinct approaches need comparison. Triggers on "debate", "competing solutions", "run architects", "architecture debate". Invocation /debate [count] "problem"
---

# Debate

N independent architect agents propose solutions, then debate through cross-pollination rounds until convergence.

## Triggers

- "debate", "run a debate", "architecture debate"
- "competing solutions", "compare approaches"
- "run N architects on this"
- Any architectural decision where independent perspectives add confidence

## Process

1. **Load team framework** — Use the Skill tool to call `/team`. This loads the team lifecycle framework for persistent teams.

2. **Parse input** — Extract agent count and problem from user input.
   - `/debate "how should we organize media by content type?"` — 3 agents (default), 5 rounds (default)
   - `/debate 5 "how should we organize media by content type?"` — 5 agents
   - Count is always the first argument if numeric. Everything else is the problem.

3. **Build the frame** — The frame is the user's problem, NOT a technical specification. Write it from the user's perspective:
   - **User story** — What the user experiences today and what they need. In their words.
   - **Business context** — Why this matters. Revenue, cost, maintenance, growth.
   - **Architectural values** — The project's principles: simplicity, elegance, maintainability. Code fails in maintenance not creation. Fewer files, fewer abstractions, simpler queries. Pull these from Claude.md files.
   - **Constraints** — Hard constraints FROM THE USER. Files to read. What's in scope, what's out. Never include your own research or conclusions — that biases agents.

4. **Assign trait seeds** — Each architect gets a lightweight lens that produces diverse initial proposals without biasing conclusions. Trait seeds describe an instinct, not a persona — a favorite area to explore, not a conclusion to reach. Assign from this roster (cycle through for N > 5):

### The UX Architect
- **Identity:** Design engineer who evaluates every technical decision through the lens of what the user sees and feels. Obsessed with the experience, not the implementation.
- **Philosophy:** The right architecture is the one that produces the best UX. If the user can't tell the difference, the engineering difference doesn't matter. Every technical choice shows up as friction or flow. Start from the interaction, work backward to the schema.
- **Known instincts:** Reaches for solutions that minimize user-facing complexity. Thinks in clicks, load times, and mental models. Suspicious of architectures that leak implementation details into the UI. "If the user has to understand our folder model to organize their images, we failed."

### The Shipper
- **Identity:** Velocity-obsessed architect who measures success in working software per week. Treats unshipped code as inventory — it depreciates, never appreciates.
- **Philosophy:** Whatever ships fastest with least risk. Proven patterns over novel ones. The best architecture is the one your team can maintain at 2am. Perfect is the enemy of deployed. Build the 80% solution, validate with real users, iterate.
- **Known instincts:** Reaches for patterns already in the codebase. Copies what works. Suspicious of novel abstractions and "elegant" solutions nobody's battle-tested. "Show me where this pattern is already used successfully in this project."

### The Maintainer
- **Identity:** Architecture purist who optimizes for the developer who inherits this code in two years. Thinks in boundaries, contracts, and dependency direction.
- **Philosophy:** Code fails in maintenance, not creation. Strict encapsulation, one-directional dependencies, small files. Every abstraction must earn its existence by reducing future complexity. If it's not trivial to maintain or rewrite, it's wrong.
- **Known instincts:** Reaches for clear module boundaries, documented contracts, explicit dependency direction. Suspicious of any change that increases coupling between modules. "If adding a content type requires touching the media module, the architecture is broken."

### The Reducer
- **Identity:** Efficiency-obsessed engineer who believes the best code is code you don't write. Counts lines, files, and abstractions like a miser counts coins.
- **Philosophy:** Fewer lines, fewer files, fewer abstractions. If the existing data model already supports what you need, stop adding things. Convention over columns. YAGNI isn't a suggestion — it's a law. Every new column, table, or file is maintenance debt that must justify its existence.
- **Known instincts:** Reaches for zero-schema-change solutions, naming conventions, existing flags. Suspicious of new columns, new tables, new abstractions. "If the convention hasn't failed yet, don't add infrastructure to prevent a hypothetical failure."

### The Polished Pragmatist
- **Identity:** Senior architect with the taste of a designer and the instincts of a principal engineer. Synthesizes competing concerns into solutions that feel inevitable — simple enough that you wonder why anyone considered anything else.
- **Philosophy:** Elegance is the intersection of simplicity and completeness. The right solution handles every edge case without looking like it handles any. Complexity is a smell; if the solution needs extensive documentation, it's the wrong solution. The database should tell the truth, the code should be boring, and the UX should be invisible.
- **Known instincts:** Reaches for the solution with the fewest moving parts that still covers all cases. Synthesizes ideas from other approaches. Suspicious of both over-engineering AND under-engineering. "If you're debating whether to add a column, the answer is in the user stories — not the schema."

5. **Build the architect prompt** — One prompt per architect, identical except for the trait seed. Structure:

```
You are one of N independent architects debating a problem. You will propose solutions, then participate in debate rounds.

Your instinct: {trait seed — one sentence}. This is a starting lens, not a conclusion. Follow the evidence in the code.

## Frame

{frame from step 3 — user story, business context, architectural values, constraints}

## Your Task

1. Read the codebase files listed in the constraints
2. Write 3-5 concrete user stories or user behaviors that any solution must support. These are YOUR stories — don't coordinate with other architects. Think about edge cases, not just happy paths.
3. Propose exactly 5 architecturally distinct solutions. For each:
   - Name and 2-3 sentence description
   - How it handles each of YOUR user stories
   - Pros and cons
   - Confidence percentage
4. Then WAIT — the facilitator will send you other architects' proposals for debate rounds.
```

6. **Create team and dispatch** — Use the /team skill. Use TeamCreate. Spawn N architect agents as persistent teammates (subagent_type: `architect`). Name them `architect-a`, `architect-b`, etc. Dispatch all in parallel.

7. **Collect proposals** — Wait for all architects to return their proposals + user stories.

8. **Run debate rounds** — Up to 5 rounds. Each round:
   - Compose a cross-pollination message for each architect containing the OTHER architects' FULL positions (not your summary — your interpretation biases them)
   - Include specific forcing questions (see round templates below)
   - Send to all architects simultaneously via SendMessage
   - Wait for all responses

9. **Early termination** — If all architects converge within 10% confidence on the same approach after any round, skip remaining rounds.

10. **Synthesize and report** — After final round, present:
    - **Consensus** — items all architects agreed on
    - **Winner** — name, description, averaged confidence, pros/cons
    - **Runner-up** — name, key difference from winner
    - **Key insight** — the single discovery that most influenced the outcome
    - **Unresolved** — anything architects couldn't agree on

## Round Templates

**Round 1 — Critique & Group**
```
Here are the other architects' proposals and user stories:
{full proposals from other architects}

1. Which proposals across all architects are the same idea? Group them.
2. Which user stories from other architects did you miss? Do your proposals handle them?
3. What's the strongest argument AGAINST your top pick?
4. Which proposal from another architect is the biggest threat to yours? Why?
5. Update your confidence for all distinct approaches.
```

**Round 2 — Narrow to top 3**
```
Round 1 responses:
{full Round 1 responses from other architects}

The field is narrowing. Here are the top contenders with averaged confidence:
{top 3-5 approaches with confidence from each architect}

1. Can you propose a HYBRID that combines the best of the top 3? Or argue why one dominates?
2. What's the minimum viable version of your preferred approach?
3. Final ranking of the top 3 only.
```

**Round 3 — Sharpen**
```
Round 2 responses:
{full Round 2 responses from other architects}

The debate has narrowed to {2-3} finalists:
{finalist descriptions with confidence}

The sharpest tension is: {describe the core disagreement}

Defend or concede. Address the specific tension directly. State your final confidence.
```

**Round 4-5 — Resolve (if needed)**
```
Round {N-1} responses:
{full responses}

{Pose the specific unresolved question that's preventing convergence}

Final answer. State your position and confidence.
```

## Key Rules

- **Agents are independent** — each reads code themselves. No pre-digested findings from you.
- **Frame leads with the USER** — not the technical mechanism. Solutions that don't serve the user are wrong regardless of elegance.
- **Route, don't interpret** — cross-pollination messages contain other architects' FULL positions, not your summary. Your interpretation biases them.
- **Force engagement** — "Which proposal threatens yours?" not "What do you think?" Vague questions get vague answers.
- **Narrow progressively** — Round 1: all proposals. Round 2: top 3. Round 3+: finalists.
- **User stories are ammunition** — architects write user stories independently, then use each other's stories to stress-test proposals in debate rounds.
