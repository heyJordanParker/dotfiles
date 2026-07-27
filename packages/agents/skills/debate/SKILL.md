---
name: debate
description: Run N independent architect Agents debating Architectural options through rounds. TRIGGER when the Architect wants multiple competing solutions evaluated, when 3+ distinct approaches need comparison, or when the Prompt says "debate", "competing solutions", "run architects", "architecture debate", or `/debate [count] "problem"`.
disable-model-invocation: true
---

# Debate

N independent Architect Subagents propose solutions, then debate through rounds until convergence.
Dispatch and resume per /subagents; `SendMessage({to: agentId})` carries the rounds.

## 1. Parse input

The first numeric argument is the Agent count. Default to 3 Agents and 5 rounds. Everything else is the problem.

Example: `/debate "how should we organize media by content type?"` means 3 Agents.
Example: `/debate 5 "how should we organize media by content type?"` means 5 Agents.

## 2. Build the Frame

The Frame is the User's problem from the User's perspective, not a technical mechanism.

Template:
    User story: what the User experiences and needs, in the User's words.
    Business: revenue, cost, maintenance, growth.
    Architecture Principles: the project's Principles from Claude.md, including simplicity, elegance, maintainability, and code fails in maintenance not creation.
    Prompt text: files to read, scope in, scope out.

Never: include your own research or conclusions in the Frame; that biases the Agents.

### The Frame leads with the User
Solutions that do not serve the User are wrong regardless of elegance.

## 3. Assign Architect Frames

Each Architect gets a one-sentence instinct that produces diverse initial Proposals. Cycle through the roster when N is greater than 5.

- User Experience Architect — Frame: User experience engineer who evaluates every technical Decision through what the User sees and feels. Principles: the right Architecture produces the best User experience; if the User cannot tell the difference, the engineering difference does not matter; every technical choice shows up as friction or ease; start from the interaction and work backward to the schema. Instincts: minimize User-facing complexity; think in clicks, load times, and mental models; distrust Architectures that leak implementation details into the user interface.
- Shipper — Frame: velocity-obsessed Architect who measures success in working software per week and treats unshipped code as inventory that depreciates. Principles: ship fastest with least risk; proven patterns over novel ones; the best Architecture is the one the team can maintain at 2 a.m.; build the smallest complete solution, validate with real Users, iterate. Instincts: reach for patterns already in the codebase; copy what works; distrust novel abstractions and untested elegant solutions.
- Maintainer — Frame: Architecture purist who optimizes for the developer who inherits the code in two years. Principles: code fails in maintenance, not creation; strict encapsulation; one-directional dependencies; small files; every abstraction must earn its existence by reducing future complexity; if it is not trivial to maintain or rewrite, it is wrong. Instincts: reach for clear module boundaries, documented contracts, and explicit dependency direction; distrust changes that increase coupling between modules.
- Reducer — Frame: efficiency-obsessed engineer who believes the best code is code not written. Principles: fewer lines, fewer files, fewer abstractions; if the existing data already supports what is needed, stop adding things; convention over columns; "you are not going to need it" is law; every new column, table, or file is maintenance debt that must justify itself. Instincts: reach for zero-schema-change solutions, naming conventions, and existing flags; distrust new columns, new tables, and new abstractions.
- Polished Pragmatist — Frame: senior Architect with the taste of a designer and the instincts of a principal engineer. Principles: elegance is the intersection of simplicity and completeness; the right solution handles every edge case without looking like it handles any; complexity is a smell; the database should tell the truth, the code should be boring, and the User experience should be invisible. Instincts: reach for the solution with the fewest moving parts that still covers all cases; synthesize ideas from other approaches; distrust both over-engineering and under-engineering.

## 4. Build the Architect Prompt

Write one Prompt per Architect, identical except the Frame.

Template:
    You are one of N independent Architects debating a problem. You will propose solutions, then participate in debate rounds.

    Your Frame: {one sentence}. This is a starting point, not a conclusion. Follow the code.

    ## Frame

    {Frame from step 3 — User story, Business, Architecture Principles, Prompt text}

    ## Your Task

    1. Read the codebase files listed in the Prompt.
    2. Write 3-5 concrete User stories or User behaviors that any solution must support. These are your stories; do not coordinate with other Architects. Think about edge cases, not just happy paths.
    3. Propose exactly 5 Architecturally distinct solutions. For each: name; 2-3 sentence description; how it handles each of your User stories; pros and cons; confidence percentage.
    4. Then wait. The facilitator will send you other Architects' Proposals for debate rounds.

### Agents are independent
Each Agent reads code itself. No pre-digested findings from you.

## 5. Dispatch the Architects

Spawn N `architect` Subagents in one message, per /subagents.

### Keep each Architect's agentId with its Frame
Every round is addressed by agentId, so record which Frame each returned id belongs to as the dispatches come back. A lost id costs that Architect's whole position; recover it from the session's `subagents/*.meta.json` rather than dispatching a fresh Architect into a debate it did not start.

## 6. Collect Proposals

Wait for every Architect to return Proposals and User stories.

## 7. Run debate rounds

Run up to 5 rounds. Each round composes one Prompt per Architect containing the other Architects' full positions plus the round's forcing questions. SendMessage every Architect by agentId in one message, then wait for all responses.

### Route, do not interpret
Round Prompts carry other Architects' full positions, not your summary. Your interpretation biases them.

### Force engagement
Ask "Which Proposal threatens yours?", not "What do you think?" Vague questions get vague answers.

### Narrow progressively
Round 1 covers all Proposals. Round 2 narrows to the top three. Round 3 and later use the finalists.

### User stories are ammunition
Architects write User stories independently, then use each other's stories to stress-test Proposals.

Template:
    Round 1 — Critique and group

    Here are the other Architects' Proposals and User stories:
    {full Proposals from other Architects}

    1. Which Proposals across all Architects are the same idea? Group them.
    2. Which User stories from other Architects did you miss? Do your Proposals handle them?
    3. What is the strongest argument against your top pick?
    4. Which Proposal from another Architect is the biggest threat to yours? Why?
    5. Update your confidence for all distinct approaches.

Template:
    Round 2 — Narrow to top three

    Round 1 responses:
    {full Round 1 responses from other Architects}

    The field is narrowing. Here are the top contenders with averaged confidence:
    {top 3-5 approaches with confidence from each Architect}

    1. Can you propose a combined approach that takes the best of the top three? Or argue why one dominates?
    2. What is the smallest complete version of your preferred approach?
    3. Give a final ranking of the top three only.

Template:
    Round 3 — Sharpen

    Round 2 responses:
    {full Round 2 responses from other Architects}

    The debate has narrowed to {2-3} finalists:
    {finalist descriptions with confidence}

    The sharpest tension is: {describe the core disagreement}

    Defend or concede. Address the specific tension directly. State your final confidence.

Template:
    Round 4-5 — Resolve if needed

    Round {N-1} responses:
    {full responses}

    {Pose the specific unresolved question that prevents convergence}

    Final answer. State your position and confidence.

## 8. Stop early on convergence

If all Architects converge within 10% confidence on the same approach after any round, skip the rest.

## 9. Synthesize and report

Report Consensus for items all Architects agreed on; Winner with name, description, averaged confidence, pros, and cons; Runner-up with the key difference; Key insight for the single finding that most influenced the outcome; and Unresolved for anything Architects could not agree on.
