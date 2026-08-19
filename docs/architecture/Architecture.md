# Prompt Architecture

The Architect-approved architecture for the prompt system: agents, skills, hooks, commands, rules, and the artifacts Agents produce. `packages/agents/skills/cc/SKILL.md` is the working law that applies this architecture; this file is the architecture itself.

## The allowance per Prompt file

- **`packages/agents/agents/<name>.md`** — description gate, config frontmatter, one Frame, its Principles. No Process — every Process is a Skill the agent names; the generator (`scripts/agents.py`) inlines the named Skill into the codex artifacts, deterministic composition in code.
- **`packages/agents/skills/<name>/SKILL.md`** — three-slot description, one Process as ordered steps, each step its Rules (with Conditions as needed), Examples, and Templates. Steps stay in SKILL.md; anything a step consults — a table, catalog, Template, subprocess — is a Reference behind a pointer, and a Process another Skill owns is named, never restated. A `disable-model-invocation` Skill can name others but cannot be named: the Architect fires it, Agents cannot use it.
- **`packages/agents/skills/<name>/references/<problem>.md`** — what one pointer names. Unreached References are deleted.
- **`packages/agents/commands/<name>.md`** — a Skill's shape; the Architect fires it.
- **`packages/claude/rules/<topic>.md`** — Rules logically grouped by purpose. The `paths:` glob is the file's centralized Condition; any individual Rule may carry its own Condition when it needs one ("When X:", markdown only, no XML) — less common precisely because the glob already scoped the file. A Condition may also name a Skill when the Rules behind it are a whole Process. A rules file may open with the Principles its Rules serve, written to the Agent as "you", before the first Rule.
- **`packages/agents/hooks/<module>.py`** — per-turn and at-action Rules. Deterministic Hook = one Rule, is the Rule; LLM Hook = that event's Rules batched; fail-open Hooks keep a prose fallback.
- **`Claude.md`** — the folder's WHY, and it loads the relevant references: its Decisions (`docs/architecture/decisions/`), a Design.md, whatever record the WHY rests on. Never Rules, Process, vocabulary, or inline Decisions.
- **`Domain.md`** — the domain's words, nothing else.

## Rules call Skills, and never depend on them

Rules are the main system and Skills are its subsystems. A Rule calls a Skill — `Use /show-me`, `Propose options via /pcc` — and the Agent goes and runs that Process. A Rule never depends on a Skill: it does not take its wording, its definitions, or any part of its own instruction from one, because a Rule must be obeyable exactly as written by an Agent that never uses the Skill.

Skills depend on Rules freely. A Skill names a Rule, builds on it, and is written to serve it.

## Artifacts outside Claude.md and SKILL.md

1. **Decisions** — `docs/architecture/decisions/000N-<the-decision>.md`; situation, choice, why, one to three sentences, plus measured scores when an experiment produced them. The Agent proposes Decisions; only the Architect makes them — typically through /interview.
2. **Shaping** — `docs/shaping/<feature>/` (moved from `~/.claude/shaping/`): frame.md, shaping.md (`shaping: true`), affordances.md (`modeling: true`), slices.md, V<N>-plan.md, research-<topic>.md. Frontmatter types the artifact; the sync Hook fires on the frontmatter, so the move costs the Hook nothing.
3. **Plans** — `docs/plans/<kebab-name>-V<N>.md` (moved from `~/.claude/plans/`); versions are new files, never overwrites. The plan-quality rules glob follows the move.
4. **Evidence** — `docs/agents/<YYYYMMDD>-<task-slug>/`: one directory per Evidence record, report.md plus screenshots beside it. Scoped to the Task, not to a plan — an Agent producing Evidence off a Proposal or a bare Prompt writes to the same place with the same shape. When a plan, Slice, or Skill step demands Evidence, it names the expected directory, and the completion gate's deterministic check is that the named report.md exists non-empty.
5. **Session state** — sessions root, `<id>/state.json`, Subagents nested.
6. **The experiment record** — the recorded prompt experiments and their scores: the Evidence behind the prompt Decisions. `docs/architecture/prompt-experiment-findings.md` and `docs/architecture/prompt-score-ledger.md`. Append-only.

## One home per block

A block has exactly one home. Every other Prompt that needs it names that home instead of carrying a copy, so Skills compose: a Process is written once, changes in one place, and any step that needs it says `/skill-name`. Both Harnesses take the named Skill whole, including its `!` autorun output.

| block | home |
|---|---|
| Frame, Principles | agent file; Principles shared by every Agent open the rules file whose Rules they serve |
| Rule | rules file (grouped by purpose), Hook (per-turn/at-action), or Skill step (part of a Process) — with a Condition wherever needed |
| Example, Template | beside its Rule or step |
| Condition | the glob for a whole rules file; inline on any Rule; on a Skill step; or a rules-file line naming a Skill |
| Process | Skill / Command / Reference, named by every step that needs it |
| WHY | folder's Claude.md, loading its references |
| domain language | Domain.md |
| Decision | docs/architecture/decisions/ |
| Shaping, Plans | docs/shaping/<feature>/, docs/plans/ |
| Evidence | docs/agents/<YYYYMMDD>-<task-slug>/ |
| session state | sessions root <id>/state.json |
