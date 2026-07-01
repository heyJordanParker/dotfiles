# Agent Instruction

The shared vocabulary for shaping how AI Agents behave.

## Philosophy

**WHO**:
Strategic question. The people a product serves: the user, architect, customer, or affected party. Knowing the WHO lets the Agent work from the customer's perspective instead of guessing from the code.

**WHY**:
Strategic question. The motivation and philosophy behind a decision. Knowing the WHY lets the Agent keep choosing in the same direction instead of fixating on details.

**WHAT**:
Strategic question. The thing being built: the building blocks, the big decisions, the slices of the work.

**HOW**:
Tactical question. The execution that produces the output: the code, the copy, the actual work.

## Agentic Engineering

**Agent**:
Autonomous, state-of-the-art AI, extremely smart, defined by a Frame and a Prompt, working toward a Goal across a conversation.
_Avoid_: LLM, model, assistant, bot, AI

**Subagent**:
Agent spawned by another Agent to do one or more Tasks.

**Orchestration**:
Operating mode where the Agent does not execute Tasks itself but hands them to Subagents, staying responsible for the Goal and for critically verifying their results.

**Architect**:
The expert software architect directing the Agent. The Architect owns the architecture and the strategic decisions. The Architect does not read or know the code, only the architecture. The Architect prompts the Agent, and the Agent owns all the tactical work.

**User**:
The person the Agent's work serves. The Goal of the work is to serve the User, so the User's needs define the work.

**Goal**:
The verifiable outcome the Agent must reach to solve a problem for the User as set by the Architect. The Agent remembers it, works toward it, and never changes it.

**Task**:
The work the Agent does to reach a verifiable result, one step toward the Goal.

**Verification**:
Undeniable proof that a Task, Goal, or Skill is finished in a way the User can consume, with no gap between the claimed result and the User's real experience.

**Frame**:
The WHO. The character played, built from one person, several people, a set of traits, or any mix. Usually accompanied by Principles.
_Avoid_: persona, role

**Principle**:
Fundamental belief that forms how an Agent thinks. When the Agent faces a decision, Principles serve as rules of thumb that simplify the choice and make the right one obvious.
_Avoid_: value, guideline, tenet

**Disposition**:
The imperfect way an Agent behaves on its own. Fixed per model, because models are frozen and do not learn. We alter the Disposition with Prompts.

**Context**:
The runtime memory of an Agent. Prompts and Agent messages and operations collect in it. The fuller it gets, the more likely the Agent is to make a mistake.

**Harness**:
The program that runs an Agent. It loops the Agent by feeding it Prompts, running its tools, and feeding results back, until the work is done. Claude Code and Codex are Harnesses.

**Hook**:
Automation the Harness runs at fixed points in the Agent's run, such as a tool call or the end of a turn.

**Proposal**:
The Agent's plan for reaching a Goal, optimized for the Architect to review and approve each architectural change in it.

**Decision**:
Architectural decision driven by the needs of the domain, recorded because it is hard to reverse, not obvious from the code, and a real trade-off. Holds what was chosen and why, so it never gets relitigated.
_Avoid_: ADR, architectural decision record, decision log

**Precedent**:
Pattern the repository already used to solve a similar problem. Agents start work by finding a Precedent, then following it and avoiding creativity. New patterns need the Architect's approval.

## Prompting

**Prompt**:
Instructions from the Architect to the Agent. Every documentation file is a Prompt: Claude.md, Domain.md, Skills, References, etc.
_Avoid_: message, request, query, command, instruction

**Rule**:
Tactical correction of the Agent's Disposition. Rules are specific – a specific action to do in a specific situation. The Frame and Principles solve the strategic, high-level decisions. Rules solve mostly everything else. Rules are used to correct the Agent when a) the Agent does something factually incorrect (due to lack of up-to-date info or general bias) or b) the Agent's Disposition is different than the approach preferred by the Architect.
_Avoid_: requirement, constraint, boundary, instruction

**Example**:
Concrete case attached to a Rule or a Skill. Examples always show the correct behavior, and may also show the incorrect one.

**Template**:
Fill-the-blanks Example.
_Avoid_: boilerplate, form, blueprint, stencil

**Process**:
The exact steps to follow, in order, to solve a recurring problem.
_Avoid_: SOP, workflow

**Skill**:
Process the Agent invokes to solve a specific, recurring problem.
_Avoid_: SOP, workflow

**Command**:
Skill the Architect invokes manually.

**Reference**:
Process split into its own Prompt for Progressive Disclosure.

**Progressive Disclosure**:
Splitting instructions across different Prompts so Agents get a minimal, focused set of instructions.

**Condition**:
Check that decides whether a Principle, Rule, or Skill step applies.

## Documentation

**Claude.md**:
Main form of documentation for Agents, and the home for what the code cannot show. It bridges the facts visible in the code and the architect's thinking and decisions behind them, holding the WHY of a part of the codebase. Nested by folder, so a deep file inherits the context of every folder above it.

**Domain.md**:
The documented version of the business's language: the one vocabulary the whole company already uses, written down so code, conversation, and documentation match it.
_Avoid_: glossary, dictionary
