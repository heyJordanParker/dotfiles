# Review Plan Reviewer Prompts

- This Reference builds the five Subagent Prompts for the Review Plan Skill.
- Each Template receives the full artifact content from the Skill step that read the artifacts.

## 1. Build the Completeness Prompt

Dispatch this Prompt with `subagent_type: "code-reviewer"`.

Template:
    You are reviewing Shaping, Modeling, Slicing, and Plan artifacts for completeness and accuracy.

    Artifacts:
    [paste all available artifact content]

    Process:
    1. List every requirement as R0, R1, and onward from the Shaping Prompt.
    2. For each requirement, verify that a concrete mechanism delivers it. Trace requirement to shape part to Affordance to Slice.
    3. Check for orphan requirements: any requirement with no corresponding Slice.
    4. Check for orphan Slices: any Slice with no requirement driving it.
    5. Check for implicit dependencies: whether Slice N depends on something not yet built and not in an earlier Slice.
    6. Check boundaries: whether any artifact violates a stated boundary.
    7. For each Slice, verify that it has all four acceptance criteria categories: functional, regression, dependency audit, and boundary.
    8. For each Slice, verify that it has Verification requirements.
    9. For every claim about how existing code works, such as "X function does Y" or "Z table has column W", use `/trace` to verify against the actual code. Flag any claim that cannot be verified or contradicts what the code shows.
    10. Check `git log --oneline -20` for affected directories. Report whether a recent commit already solves or partially solves what this Plan proposes.
    11. Identify every assertion about existing code behavior. For each one, state whether it is backed by a file read or is an assumption. Flag unverified assertions as "UNVERIFIED ASSUMPTION: [claim]".
    12. For each step, verify that the data and Context it needs are available at that Execution point.
    13. Check whether the Plan assumes infrastructure that may not exist: queue workers, scheduler, database tables, environment variables, vault secrets, or third-party configuration.
    14. For every public contract change, verify that every consumer is updated, including tests, seed files, and documentation.
    15. For every recommendation, verify that it cites specific code, library behavior, or Critical Path. Flag generic pros and cons without code-level Evidence.
    16. Identify sections with "TBD", deferred Decisions, or options without recommendations. Each is a gap requiring Architect input.
    17. Compare the Plan's scope to the original ask. Flag new abstractions, files, or patterns beyond what was requested as "SCOPE EXPANSION: [what was added]".

    Report Critical/Important/Minor findings.
    If clean: "All requirements covered. All claims verified."

## 2. Build the Critical Path Prompt

Dispatch this Prompt with `subagent_type: "ux-tester"`.

Template:
    You are reviewing Shaping, Modeling, Slicing, and Plan artifacts for User experience completeness. Can a real User actually use everything being built?

    Artifacts:
    [paste all available artifact content]

    Process:
    1. Identify every new feature, page, or interaction the Plan introduces.
    2. For each one, trace the complete Critical Path.
    3. Verify the entry point: how the User gets to it, and whether there is a link, button, or menu item.
    4. Verify the steps: what the User does at each step, and what they see.
    5. Verify the exit: where the User ends up, and whether the end state is clear.
    6. Check for dead ends: features built but unreachable from the existing User Interface.
    7. Check missing error states: what the User sees when something fails.
    8. Check missing empty states: what the User sees before data exists.
    9. Check missing loading states: what the User sees while data loads.
    10. Verify every new feature or page has an explicit Plan to make it reachable from the existing User Interface. A feature with no navigation path is dead on arrival.

    Report Critical/Important/Minor findings.
    If clean: "All Critical Paths are complete and reachable."

## 3. Build the Regressions Prompt

Dispatch this Prompt with `subagent_type: "code-reviewer"`.

Template:
    You are reviewing Shaping, Modeling, Slicing, and Plan artifacts for regression risk. For every file the Plan modifies, determine what could break.

    Artifacts:
    [paste all available artifact content]

    Process:
    1. For every file listed as modified in the Plan, read the actual file in the codebase.
    2. Identify all existing behavior: what the file currently does and what depends on it.
    3. Use `/trace` to find all callers and importers of modified functions, classes, components, or exports.
    4. For each existing behavior, verify whether the Plan preserves it and whether it could break.
    5. Check whether existing tests cover the behavior and whether the Plan includes regression checks in acceptance criteria.
    6. Check whether the Plan modifies shared utilities, base classes, or interfaces. If so, trace every consumer.
    7. For every fix or change, check the reverse direction. If fixing an A to B interaction, verify B to A still works. Pay special attention to authentication, cookie changes, shared state, platform boundaries, and tenant boundaries.
    8. Check whether modified code runs inside try-catch blocks owned by other systems. Report whether a failure here could silently prevent other listeners, Hooks, or middleware from executing.
    9. For any migration, renaming, or deletion, use `/trace` across the whole codebase for references to old names, old paths, or old values. Include config files, vault, continuous integration, continuous deployment, and deployment scripts.

    Report Critical/Important/Minor findings.
    If clean: "No regression risks identified."

## 4. Build the Architect Prompt

Dispatch this Prompt with `subagent_type: "architect"`.

Template:
    You are reviewing Shaping, Modeling, Slicing, and Plan artifacts for Architectural quality.

    Artifacts:
    [paste all available artifact content]

    Process:
    1. Read the nearest Claude.md files for the affected codebase. They define Rules, Precedents, and boundaries.
    2. For each new file or component the Plan creates, check whether something similar already exists, whether it follows Precedents in the same directory, and whether naming is consistent with project conventions.
    3. Check encapsulation: whether module boundaries are respected and whether the Plan reaches into internals.
    4. Check dependency direction: whether circular or wrong-direction dependencies appear.
    5. Check code reuse: whether the Plan duplicates functionality that already exists.
    6. Cross-reference against Claude.md Rules and boundaries. Report any violation.
    7. When reviewing Affordances, trace User requirements through the wiring and verify that the path tells a coherent story.
    8. When reviewing Affordances, flag incoherent wiring, missing paths, diagram-only nodes, naming resistance, stale Affordances, wrong causality, and implementation mismatch.
    9. Apply the Naming Test to each Affordance: identify the caller, the step-level effect, and the one idiomatic verb. If the name needs "or" to connect two verbs, it likely bundles two Affordances. If the name matches a downstream effect, it names the chain instead of the step.
    10. For every external library or package the Plan uses, verify that the public surface matches what the Plan assumes. Check that methods, parameters, and behaviors actually exist.
    11. List every Architectural Decision the Plan makes. For each one, state whether it is made and justified or deferred to implementation. Flag deferred Decisions as "DEFERRED: [Decision]".
    12. Check whether the Plan builds foundational or risky pieces first. If a later Slice could invalidate earlier ones, flag it as "ORDERING RISK: Slice N depends on unvalidated approach in Slice M".
    13. Flag any change that alters observable behavior, including error handling, redirect targets, response shapes, or authentication Critical Path, even if described as a "refactor" or "cleanup". Mark it as "BEHAVIORAL CHANGE: [description]".

    Report Critical/Important/Minor findings.
    If clean: "Architecture is sound."

## 5. Build the Frontend Prompt

Dispatch this Prompt with `subagent_type: "frontend-engineer"`.

Template:
    You are reviewing Shaping, Modeling, Slicing, and Plan artifacts for frontend Architecture and implementation quality.

    Artifacts:
    [paste all available artifact content]

    Process:
    1. Read the nearest Claude.md files for frontend Rules.
    2. Read three or more existing components in the same directory or module as the Plan's new components.
    3. Check frontend Architecture: component hierarchy, state management Precedents, data movement Precedents, and routing conventions.
    4. Check component reuse: whether the Plan creates new components where existing ones could be extended.
    5. Check data movement: whether calls, queries, and mutations follow project Precedents.
    6. For each new frontend component, verify naming, file layout, props, styling approach, state management, and interaction patterns against existing component Precedents.
    7. Check for missing interactive states: hover, focus, active, and disabled.

    Report Critical/Important/Minor findings.
    If clean: "Frontend Architecture and Precedents are consistent."
