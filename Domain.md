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

**Orchestrator**:
The Agent running Orchestration. Responsible for the approved delivery end to end. Quality is judged against the User, the Architecture, and the business.
_Avoid_: coordinator, main agent, lead

**Owner**:
Who is responsible for something. Everything has exactly one Owner. The Owner makes every decision about it and everyone else carries feedback to the Owner instead of deciding directly.
_Avoid_: assignee, responsible party, stakeholder

**Architect**:
The person who directs the Agents. The Architect decides the Capabilities and the UX, and reads the Architecture, never the code.

**Architecture**:
The big-picture decisions of a system: the files, the public APIs, the database, and the names that form the application's language. Later code builds on it, so it is costly to change.
_Avoid_: design, structure

**Module**:
A self-contained part of the codebase that owns one or more Capabilities. Other code reaches it only through its Public API.
_Avoid_: component, building block, service, surface, boundary, seam

**Public API**:
What other code may use from a Module. Changing it is an Architectural change, and everything behind it is tactical.
_Avoid_: interface, contract, surface, affordance

**User**:
The person the Agent's work serves. The Goal of the work is to serve the User, so the User's needs define the work.

**Critical Path**:
The application functionality that's critical to the User. It's the reason WHY the user bought the application & decides whether the product is worth paying for. ANY issues there lose Users and revenue directly and outrank all other work in priority.
_Avoid_: flows, critical functionality, user journeys

**Goal**:
The outcome the User wants. The Architect sets it. The Agent reaches it, verifies it, and never changes it.

**Problem**:
Something that stops the User from reaching a Goal, or slows down work on the project. A Problem is about the Architecture or a Capability: one the product should have and does not, or had and lost. What the code does is not a Problem until it changes one of them.

**Solution**:
One of several ways to solve a Problem. Its tradeoffs and our context decide if it is the best one. It requires one or more Capabilities.

**Capability**:
Something our product can do for the User.

**Process**:
The exact steps to follow, in order, to predictably produce a result. A Capability requires one or more Processes.
_Avoid_: SOP, workflow

**Task**:
The work the Agent does to reach a verifiable result, one step toward the Goal.

**Slice**:
Vertical group of work that cuts through every layer and ends in something demo-able. A plan is broken into Slices, and Slices with no dependency between them are intended to execute in parallel.
_Avoid_: wave, phase, milestone, batch

**Shaping**:
The Architect and an Agent discussing a rough form of the final deliverable. The discussion surfaces the non-negotiable requirements and boundaries that the Execution must respect. The shape stays rough on purpose. It leaves room for judgment during Execution while the requirements and boundaries act as guard rails.

**Modeling**:
Turning the shape selected in Shaping into the concrete affordances of the change. Modeling extracts further context from the Architect and finds the correct scope for the Execution. The model shows the database, the UX, and the Architecture, and every decision after Modeling is made against the model.

**Affordance**:
A thing the Agent is allowed to act on. UI Affordances are what the User sees and interacts with. Code Affordances are Architectural: the files, the APIs, and the database changes behind the UI.

**Slicing**:
Breaking the modeled change into Slices that are executed and verified separately. Separate Slices let the Architect review specific increments, let the Plan change midway, and limit how much work is lost when something goes wrong.

**Review**:
The Architect examining and correcting the Execution. The Agent requests a Review by presenting the work following the Problem Hierarchy.

**Problem Hierarchy**:
All work solves the Problems that block a Goal, and those Problems form a tree. A big Problem causes smaller Problems, which cause smaller ones in turn. Solving a Problem removes every Problem it causes, so solving from the top removes whole branches at once. Independent branches are worked in parallel.

**Verification**:
Undeniable proof that a Task, Goal, or Skill is finished in a way the User can consume, with no gap between the claimed result and the User's real experience. Verification guarantees the Goal was achieved before any Review starts. A Review that finds the Goal unmet is a failed Verification.

**Evidence**:
Artifact an Agent produces to prove its work in the form of a report with (if needed) accompanying screenshot(s). Other Agents reference Evidence later instead of trusting the Agent's claims blindly. Agent work is incomplete without Evidence.
_Avoid_: proof, QA record

**Frame**:
The WHO. The character played, built from one person, several people, a set of traits, or any mix. Who the Agent is biases it toward that character's best practices. Usually accompanied by Principles.
_Avoid_: persona, role

**Principle**:
Fundamental belief that forms how an Agent thinks. When the Agent faces a decision, Principles serve as rules of thumb that simplify the choice and make the right one obvious.
_Avoid_: value, guideline, tenet

**Disposition**:
The imperfect way an Agent behaves on its own. Fixed per model, because models are frozen and do not learn. We alter the Disposition with Prompts.

**AI Slop**:
Work from an Agent that looks complete but is impractical or flat-out incorrect. Even with good Prompting, AI Slop appears on every layer (choices, architecture, code, comments, docs, orchestration, thoroughness) and must be removed everywhere. AI Slop is costly in three ways: by making the product look like a cheap commodity instead of a premium solution, by creating debt that future Agents must compensate for (reducing future Agent quality), or by shipping incomplete work and regressions that frustrate the User and lose the business money.
_Avoid_: slop, low-quality output, boilerplate

**Elegant**:
A system that achieves all its goals with the fewest parts. It reuses parts instead of adding new ones, achieves every result through only one path, and cannot remove a part without losing a Capability.
_Avoid_: clever, sophisticated, clean

**Context**:
The runtime memory of an Agent. Prompts and Agent messages and operations collect in it. The fuller it gets, the more likely the Agent is to make a mistake.

**Harness**:
The program that runs an Agent. It loops the Agent by feeding it Prompts, running its tools, and feeding results back, until the work is done. Claude Code and Codex are Harnesses.

**Hook**:
Automation the Harness runs at fixed points in the Agent's run, such as a tool call or the end of a turn.

**Proposal**:
The Agent's Solutions to the Problems that block a Goal, each shown with the Capabilities and Architecture it changes, for the Architect to review and approve before Execution.

**Plan**:
The contract for reaching a Goal. A Plan lives in a file.
_Avoid_: roadmap, spec

**Decision**:
Architectural decision driven by the needs of the domain, recorded because it is hard to reverse, not obvious from the code, and a real trade-off. Holds what was chosen and why, so it never gets relitigated.
_Avoid_: ADR, architectural decision record, decision log

**Precedent**:
Modern, established system that a whole system of ours copies.

## Prompting

**Prompt**:
Instructions for an Agent. Every documentation file is a Prompt: Claude.md, Domain.md, Skills, Rules, References, etc.
_Avoid_: message, request, query, command, instruction

**Rule**:
Tactical correction of the Agent's Disposition: a specific action in a specific situation. Principles teach the Agent how to think, and the Frame biases it toward the best practices of who it is. Rules correct only what the Agent still gets wrong under both: a fact it does not know, or a Disposition that differs from the approach the Architect prefers.
_Avoid_: requirement, constraint, boundary, instruction

**Fact**:
Declarative statement that saves research – of what the code cannot show or cannot show effectively – sparing the Agent multiple file reads, checking docs, querying APIs, or gathering information. Facts are the result of that research. Facts are never imperative (that's a Rule) or steps (that's a Process). A Fact lives in the Claude.md of the folder the Agent would need it in, or on the Skill step that needs it.
_Avoid_: note, context, info

**Example**:
Concrete case attached to a Rule or a Skill. Examples always show the correct behavior, and may also show the incorrect one.

**Template**:
Fill-the-blanks Example.
_Avoid_: boilerplate, form, blueprint, stencil

**Skill**:
Process the Agent invokes to solve a specific, recurring Problem.
_Avoid_: SOP, workflow

**Command**:
Skill the Architect invokes manually.

**Reference**:
Process split into its own Prompt for Progressive Disclosure.

**Progressive Disclosure**:
Splitting instructions across different Prompts so Agents get a minimal, focused set of instructions.

**Rule File**:
File grouping the Rules for one topic. The Harness loads a Rule File automatically when its Condition is met.

**Condition**:
Check that decides whether a Principle, Rule, or Skill step applies.

**Fluff**:
Text that does nothing for its reader: a Prompt line the Agent behaves the same without, or reply text that neither moves the Goal nor needs the Architect's eyes. Cut on sight.
_Avoid_: filler, padding

**Overprompting**:
Correcting more than the Agent needs, until the volume buries the signal and the Agent starts ignoring instructions wholesale.

## Documentation

**Claude.md**:
Main form of documentation for Agents: the WHY of a part of the codebase and the Facts its whole folder consumes. Nested by folder, so a deep file inherits the context of every folder above it.

**Domain.md**:
The documented version of the business's language: the one vocabulary the whole company already uses, written down so code, conversation, and documentation match it.
_Avoid_: glossary, dictionary
