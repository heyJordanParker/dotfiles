
# Building Skills

Guide for creating and maintaining Claude Code skills.

## Triggers

- "create a skill", "build a skill", "make a skill for..."
- "edit this skill", "update the skill", "add to skill"
- "move skill to dotfiles", "make skill personal/project"
- "how do skills work", "where do skills live"

## Principles

1. **Project first** - Check existing skills for style. Project conventions → ecosystem standards → these principles.
2. **State the core; cut what derives from it** - Write the principle the rest follows from, then delete every rule derivable from it. "Follow project patterns" beats a 20-row table. The line count is the symptom; the rules you spelled out instead of cutting are the cause.
3. **YAGNI** - Abstract after duplication, not before.
4. **Capture the "why"** - Examples need reasoning. "Bad: X, Good: Y, Why: Z"
5. **Prefer examples to prose** — When defining acceptable patterns, use good/bad code examples instead of prose descriptions. The example shows what to do; the reasoning explains why it matters.
6. **Progressive disclosure** - SKILL.md contains everything the agent needs for the skill's core job. If something is needed 80%+ of the time, it belongs in SKILL.md. References are for sub-tasks that only sometimes apply — the agent may never open them. Group reference content by what gets used together.
7. **Ground in real code** - Explore codebase for actual patterns.
8. **Check existing docs** - Claude.md files, README, linter configs may have conventions.
9. **Validate incrementally** - Get key decisions approved first.
10. **Definition of done** - Every skill needs a validation checklist.
11. **Prose gives the reason; the hook blocks the violation** - When a hook deterministically blocks a failure, state the rule and its reason once — enough for the agent to comply before it hits the hook, and to apply the rule where the hook can't reach. Never re-list the specific cases the hook already catches; they are blocked whether the prose names them or not, so the restatement only adds load.

## Locations

- **Personal:** `~/.claude/skills/skill-name/` — General skills across all projects
- **Project:** `.claude/skills/skill-name/` — Codebase-specific skills
- **Plugin:** `plugin/skills/skill-name/` — Bundled with plugins

## Before

1. **Understand Scope** - Use AskUserQuestion. One question at a time, prefer multiple choice.
2. **Research** - Read existing skills for patterns. Check Claude.md files.
3. **Explore Related Skills** - Find skills that overlap or complement. Prevent duplicates. Plan cross-references.
4. **Decide Placement** - Personal (general) vs Project (codebase-specific). Infer from context.
5. **Key Decisions First** - Get approval on structure before writing.

## During

**Write Files**

Structure skill execution in phases:
1. **Before** - What does the skill need to prepare? Prerequisites, research, setup.
2. **During** - The actual work. Steps, decisions, guidance.
3. **After** - Validation. Checklists, verification, cleanup.

**Avoid Stale Content** - You love hardcoding file lists. Don't. They go stale immediately.

- Mention only critical components by name
- Instruct the skill to use codebase exploration (Glob, Grep, git ls-files) to derive lists dynamically
- Never enumerate files, dependencies, or structure inline

**Bad:** "This project has: src/auth.ts, src/api.ts, src/utils.ts"

**Good:** "Run `git ls-files 'src/*.ts'` to see current source files"

**Write Direct** — skill content tells the agent what to do differently. Never explain capabilities the agent already has (what tools do, how they work, why one is generally better). If the agent already knows it, the line wastes tokens.

**Bad:** "Grep finds where something is. Reading tells you what it does, why it exists, and what breaks if you change it."
**Good:** "Use Read for research, not Grep."
**Why:** The agent knows what Grep and Read do. The skill tells it which to use, not what they are.

Keep files under 100 lines — split or trim if longer. "Use X" not "consider using X."

**Embed auto-run context** — a line whose content is a backtick-wrapped command prefixed with `!` runs at skill load and inlines its stdout into the skill, with no agent action. Use it to put live state in front of the agent automatically instead of telling the agent to go fetch it.

````markdown
## Current Changes

!`git changes`
````

The agent reads the output already there; it never runs the command itself. The `/commit` skill front-loads `!git changes`, `!git diff HEAD`, and `!git log --oneline -10` this way so the diff and history are in context before it writes the message. Reach for project status aliases (`git changes`) and `trace` over raw git. This runs at expansion time, ahead of the agent's Bash tool, so the trace/git guard hooks do not intercept it.

Skill content can reference `${CLAUDE_EFFORT}` for the active effort level (v2.1.120+). In command bodies, escape a literal `$` before a digit as `\$` so it isn't read as an argument placeholder (v2.1.163+). The `disableSkillShellExecution` setting turns off inline `!` shell execution in skills, custom slash commands, and plugin commands (v2.1.91+).

**Structure References** — a reference is an optional read; an overconfident agent skips it even when it should open it. So the main doc carries everything the agent must have to get the output right; a reference is a Process for one specific hard action, opened only when the agent commits to that action. If a skipped reference changes the output, it was in the wrong file.

- **The 80% test:** "Does the agent need this for 80%+ of invocations?" If yes → SKILL.md. If no → reference for that specific sub-task
- A reference is a Process for one hard action (the live-browser setup sequence, the plugin-publish steps), never background the agent is trusted to read first
- Name after what you're DOING: `building-skills.md` — not what the topic IS: `context-engineering.md`
- **Litmus test:** verb phrase = process. "building skills" ✓. "context engineering" ✗.
- **Compose, don't duplicate:** when a step is already documented in another skill, reference that skill instead of copying its steps. A variant or sub-procedure of the main skill that isn't worth its own skill becomes another reference, never inline bloat.

**Route by usecase, not component** — Agents open references when solving a specific problem. Structure routing to match the agent's mental state, not the API surface.

Bad — routes by component (agent must already know the answer):
```markdown
- [hooks.md](references/hooks.md) — hook system
- [rules.md](references/rules.md) — rules system
```

Good — routes by problem (agent finds their situation):
```markdown
- Optimizing a query → optimizing-queries.md
- Preventing agent mistakes → enforcing-guardrails.md
- Deciding whether to use an effect → effects.md
```

Why: component routing requires the agent to translate "I need to prevent X" → "that sounds like hooks." Usecase routing eliminates that translation step. Reference filenames stay as verb phrases (`optimizing-queries.md`), but routing sentences describe the problem, not the tool.

Apply this to SKILL.md Topics, rules files that route to references, and any section pointing agents to further reading.

**Signs structure is wrong:**
- Agent produces wrong output despite rules existing → rules are in a reference instead of SKILL.md
- Agent always reads files A and B together → merge into one file
- Agent loads file, uses <10% → split along task boundaries
- Same info in multiple files → move to common parent
- Agent can't complete task without opening a reference → that content belongs in SKILL.md

## After

- [ ] Description clear about when to use? (≤1024 chars)
- [ ] Principles (adaptable) not rules (brittle)?
- [ ] Everything needed 80%+ of invocations is in SKILL.md?
- [ ] Every reference is a sub-task that only sometimes applies?
- [ ] Every example includes the why?
- [ ] Understandable without prior context?
- [ ] Has validation checklist?
- [ ] Related skills identified and cross-referenced?

## Editing Skills

### Update Content

1. Read current skill files **in full** — skills are holistic documents where piecemeal edits cause contradictions and drift
2. Identify what to change
3. Follow principles in SKILL.md and "Write Direct" guidance above

### Add References

Format in SKILL.md:
```markdown
## References

- [reference.md](references/reference.md) - Description
- [examples.md](references/examples.md) - Description
```

### Rename/Move

1. Rename directory to new name
2. Update `name:` in frontmatter to match directory
3. Update any commands that reference the skill
4. Restart Claude Code

### Move Personal ↔ Project

- **To personal:** Move from `.claude/skills/` to `~/.claude/skills/`
- **To project:** Move from `~/.claude/skills/` to `.claude/skills/`

## File Structure

```
skill-name/
├── SKILL.md      # Required - must be named exactly "SKILL.md"
├── reference.md  # Optional - lookup/technical specs
├── examples.md   # Optional - good/bad examples
├── scripts/      # Optional - utility scripts
└── templates/    # Optional - templates
```

## Frontmatter Fields

```yaml
---
name: skill-name
description: When to use this skill (≤1024 chars)
allowed-tools:  # Optional - restricts available tools
  - Read
  - Grep
  - Glob
---
```

**Required:**
- **name:** ≤64 chars, lowercase, hyphens, numbers — must match directory name
- **description:** ≤1024 chars — critical for auto-activation, be specific about triggers. Never leave it empty: the `available_skills` listing falls back to the skill BODY when `description` is blank, so an empty description leaks the entire body into the listing every session (the opposite of hiding the skill)

**Common:**
- **allowed-tools:** Tools available while the skill is active. Comma-separated string or YAML list
- **disallowed-tools:** Tools removed from the model while the skill is active; cleared when the user sends the next message. Comma-separated string or YAML list. The camelCase `disallowedTools` is the normalized alias (v2.1.152+)
- **context:** Where the skill runs — `inline` expands into the current conversation; `fork` spawns a subagent (pair with `agent:`). Enum: `inline` or `fork`
- **model:** Model the skill runs under. Aliases: `haiku`, `sonnet`, `opus`, `fable`, a full model ID, or `inherit` (match the parent conversation). `best`, `opusplan`, and the `[1m]` long-context variants also resolve
- **effort:** Thinking effort — `low`, `medium`, `high`, `max`, or an integer
- **agent:** Specify agent type for execution (e.g., `agent: code-reviewer`)
- **argument-hint:** Placeholder shown after the slash command name in the `/` menu (e.g. `argument-hint: [count] "task"`)
- **when_to_use:** Snake_case. Extra trigger guidance appended to the skill's tool description — supplements `description`
- **user-invocable:** `false` hides the skill from the `/` slash menu only — it stays in `available_skills`, so the model can still auto-invoke it. Orthogonal to `disable-model-invocation` (default: `true` for skills in `/skills/`)
- **disable-model-invocation:** `true` drops the skill from the `available_skills` listing entirely and the Skill tool refuses it for the model (`Skill <name> cannot be used with Skill tool due to disable-model-invocation`); it runs only when the user types `/<skill>`. Because it's gone from the listing, it is also NOT preloadable via an agent's `skills:` — the loader warns `Skill '<name>' specified in frontmatter was not found` and skips it (v2.1.110+)
- **hooks:** Define scoped PreToolUse/PostToolUse/Stop hooks (see automating-with-hooks.md)

**Optional:**
- **display-name:** Human-readable name shown in the UI
- **default-enabled:** Whether the skill loads by default
- **license:** For open-source skills (e.g., `MIT`, `Apache-2.0`)
- **metadata:** Custom key-value pairs (author, version, mcp-server, tags)
- **shell:** Shell for inline `!`-command blocks — `bash` (default, every platform) or `powershell`

**Internal (don't author):**
- **arguments:** Typed variant of `argument-hint` — author `argument-hint` instead
- **version:** Bookkeeping marker, not surfaced to users
- **created_by:** Provenance marker (e.g. `dream-proposal`)

**Not skill frontmatter:**
- **compatibility:** A plugin-manifest (`plugin.json`) field. The SKILL.md schema does not define it, so it is not read here (v2.1.195)

The keys `display-name`, `default-enabled`, `fallback`, and `metadata.*` accept kebab-case, snake_case, or camelCase (v2.1.186+). Malformed YAML frontmatter loads the skill body with empty metadata instead of failing silently.

**Security restrictions:**
- Avoid XML angle brackets (`<` `>`) in frontmatter by convention — frontmatter appears in the system prompt and could inject instructions. The harness does not actually reject them
- No "claude" or "anthropic" in skill names — reserved

Both syntaxes work for allowed-tools:
```yaml
# Array
allowed-tools:
  - Read
  - Grep

# Inline
allowed-tools: [Read, Grep]

# Tool-specific patterns
allowed-tools: "Bash(python:*) Bash(npm:*) WebFetch"
```

**Note**: If the skill requires external packages, list them in the description or compatibility field.

## Discovery

Claude auto-discovers skills from all three locations (personal, project, plugin). No explicit invocation needed.

**Activation**: Claude reads descriptions and decides when to use a skill based on the current task. Generic descriptions fail — be explicit about triggers and use cases.

**Phases:**
1. **Discovery**: Claude reads frontmatter to decide if skill is relevant
2. **Execution**: Claude loads supporting files only if skill activates

Comprehensive documentation doesn't bloat initial decision-making.

**Writing descriptions:**

```yaml
# Good — specific triggers, clear scope
description: Manages Linear project workflows including sprint planning,
  task creation, and status tracking. Use when user mentions "sprint",
  "Linear tasks", "project planning", or asks to "create tickets".

# Good — negative triggers prevent over-activation
description: Advanced data analysis for CSV files. Use for statistical
  modeling, regression, clustering. Do NOT use for simple data exploration
  (use data-viz skill instead).

# Bad — too vague, no triggers
description: Helps with projects.

# Bad — no trigger phrases
description: Creates sophisticated multi-page documentation systems.
```

**Debugging activation:**
- Under-triggering: add keywords, especially technical terms users would say
- Over-triggering: add negative triggers (`Do NOT use for...`), narrow scope
- Test: ask Claude "When would you use the [skill-name] skill?" — it quotes the description back, revealing gaps

**Hot reload**: Skills reload automatically when modified — no restart needed. `/reload-skills` re-scans skill directories without restarting (v2.1.152+); a SessionStart hook can do the same with `reloadSkills: true`.

**Nested discovery**: Skills in nested `.claude/skills/` directories load when working on files there. On a name clash with a higher skill, the nested one appears as `<dir>:<name>` so both stay available (v2.1.178+).

**Listing cap**: The model sees up to 1,536 chars of each `description` in the skill listing; longer descriptions are truncated with a startup warning (v2.1.105+).

**Suppressing skills**: The `skillOverrides` setting controls visibility per skill — `off` hides it from the model and `/` menu, `user-invocable-only` hides it from the model only, `name-only` collapses the description (v2.1.129+). The `disableBundledSkills` setting (or `CLAUDE_CODE_DISABLE_BUNDLED_SKILLS`) hides all bundled skills, workflows, and built-in slash commands from the model (v2.1.169+).

## Related Skills

- [claude-md.md](claude-md.md) - Template, structure, and update process for Claude.md files

## References

- [building-examples.md](building-examples.md) - Good/bad examples with reasoning

**Note:** `paths:` frontmatter (for rules and skills) accepts a single glob string or a YAML list of globs.
