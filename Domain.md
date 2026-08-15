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
The expert software architect directing the Agent. The Architect owns the Architecture and the strategic decisions. The Architect does not read or know the code, only the Architecture. The Architect prompts the Agent, and the Agent owns all the tactical work.

**Architecture**:
The big-picture decisions of a system. Architecture is the files, the public APIs, and the database. It is expensive to reverse, so every Architectural change needs the Architect's approval. Everything below Architecture is tactical and owned by the Agent.
_Avoid_: design, structure

**User**:
The person the Agent's work serves. The Goal of the work is to serve the User, so the User's needs define the work.

**Critical Path**:
The application functionality that's critical to the User. It's the reason WHY the user bought the application & decides whether the product is worth paying for. ANY issues there lose Users and revenue directly and outrank all other work in priority.
_Avoid_: flows, critical functionality, user journeys

**Goal**:
The verifiable outcome the Agent must reach to solve a problem for the User as set by the Architect. The Agent remembers it, works toward it, and never changes it.

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
A thing the Agent is allowed to act on. UI Affordances are what the User sees and interacts with. Code Affordances are Architectural — the files, the APIs, and the database changes behind the UI.

**Slicing**:
Breaking the modeled change into Slices that are executed and verified separately. Separate Slices let the Architect review specific increments, let the Plan change midway, and limit how much work is lost when something goes wrong.

**Execution**:
An Agent building what was decided with or without a Plan/Proposal. The Agent owns every tactical decision and escalates any Architectural change to the Architect.

**Review**:
The Architect examining and correcting the Execution. The Agent requests a Review by presenting the work following the Decision Hierarchy.

**Decision Hierarchy**:
Work is organized in a hierarchy. Decisions at a higher level define the decisions below them and invalidate them when they change. Deciding top-down is efficient because a few big calls at a higher level replace many small ones below. Independent branches of the hierarchy are worked in parallel.

**Verification**:
Undeniable proof that a Task, Goal, or Skill is finished in a way the User can consume, with no gap between the claimed result and the User's real experience. Verification guarantees the Goal was achieved before any Review starts. A Review that finds the Goal unmet is a failed Verification.

**Evidence**:
Artifact an Agent produces to prove its work in the form of a report with (if needed) accompanying screenshot(s). Other Agents reference Evidence later instead of trusting the Agent's claims blindly. Agent work is incomplete without Evidence.
_Avoid_: proof, QA record

**Frame**:
The WHO. The character played, built from one person, several people, a set of traits, or any mix. Usually accompanied by Principles.
_Avoid_: persona, role

**Principle**:
Fundamental belief that forms how an Agent thinks. When the Agent faces a decision, Principles serve as rules of thumb that simplify the choice and make the right one obvious.
_Avoid_: value, guideline, tenet

**Disposition**:
The imperfect way an Agent behaves on its own. Fixed per model, because models are frozen and do not learn. We alter the Disposition with Prompts.

**AI Slop**:
Work from an Agent that looks complete but is impractical or flat-out incorrect. Even with good Prompting, AI Slop appears on every layer (choices, architecture, code, comments, docs, orchestration, thoroughness) and must be continually removed and corrected everywhere. AI Slop is costly in three ways: by making the product look like a cheap commodity instead of a premium solution, by creating debt that future Agents must compensate for (reducing future Agent quality), or by shipping incomplete work and regressions that frustrate the User and lose the business money.
_Avoid_: slop, low-quality output, boilerplate

**Elegant**:
A system that achieves all its goals with the fewest parts. It reuses parts instead of adding new ones, achieves every result through only one path, and cannot remove a part without losing a capability.
_Avoid_: clever, sophisticated, clean

**Context**:
The runtime memory of an Agent. Prompts and Agent messages and operations collect in it. The fuller it gets, the more likely the Agent is to make a mistake.

**Memory**:
Persistent Agent memory. Given to orchestrating Agents.

**Harness**:
The program that runs an Agent. It loops the Agent by feeding it Prompts, running its tools, and feeding results back, until the work is done. Claude Code and Codex are Harnesses.

**Hook**:
Automation the Harness runs at fixed points in the Agent's run, such as a tool call or the end of a turn.

**Proposal**:
The Agent's proposed path to a Goal, optimized for the Architect to review and approve each Architectural change in it. A Proposal is organized as a Decision Hierarchy and presented hierarchically. A Proposal lives in the conversation.

**Plan**:
The contract for reaching a Goal. The Plan's Architecture is immutable and changing it requires the Architect's approval. Tactical decisions inside the Plan are owned by the Agent executing it. A Plan lives in a file.
_Avoid_: roadmap, spec

**Interview**:
Process where an Agent builds understanding of the Architect's requirements and context by asking questions.
_Avoid_: requirements gathering, discovery, planning session

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

**Fact**:
Declarative statement that saves research – of what the code cannot show or cannot show effectively – sparing the Agent multiple file reads, checking docs, querying APIs, or gathering information. Facts are the result of that research. Facts are never imperative (that's a Rule) or steps (that's a Process). A Fact lives in the Claude.md of the folder the Agent would need it in, or on the Skill step that needs it.
_Avoid_: note, context, info

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

**Rule File**:
File grouping the Rules for one topic. The Harness loads a Rule File automatically when its Condition is met.

**Condition**:
Check that decides whether a Principle, Rule, or Skill step applies.

**Fluff**:
Line in a Prompt that corrects nothing — the Agent behaves the same without it. Cut on sight.
_Avoid_: filler, padding

**Overprompting**:
Correcting more than the Agent needs, until the volume buries the signal and the Agent starts ignoring instructions wholesale.

## Documentation

**Claude.md**:
Main form of documentation for Agents: the WHY of a part of the codebase and the Facts its whole folder consumes. Nested by folder, so a deep file inherits the context of every folder above it.

**Domain.md**:
The documented version of the business's language: the one vocabulary the whole company already uses, written down so code, conversation, and documentation match it.
_Avoid_: glossary, dictionary
