---
name: cc
description: Use when working with Claude Code itself - skills, agents, hooks, settings, documentation. Covers building, testing, sharing skills, updating Claude.md files, and plugin distribution.
---

# Claude Code

Last synced with Claude Code **v2.1.87** (2026-03-29).

Guide for working with Claude Code's extensibility system.

## Approach

Before building any AI-driven task, decide which parts the AI should do:

1. List the steps the goal needs.
2. Give the AI only the steps that interpret human language — code reads language abysmally. Write every other step as code, because the AI is slow and non-deterministic.

## Principles

Apply to all topics below:

- **The rule** — instructions exist exclusively to correct default behavior. If Claude Code would already do it without being told, the instruction doesn't belong. This is the #1 litmus test for any line in any Claude Code configuration — Claude.md, skills, agents, hooks, rules. Can't name the default behavior it overrides? Delete it
- **Frame.** A persona an agent embodies — e.g. "Act as John Carmack writing a game engine." Frames shape baseline tone, vocabulary, and judgment across every token. One frame per prompt — a second frame splits the agent and the model averages between them.
- **Descriptions are gates** — the agent reads `description` to decide whether to load the rest. Write three slots: what it does, TRIGGER when (concrete user phrases), DO NOT TRIGGER when (adjacent phrasings that mean something else — name the alternative skill that does fire for the adjacent case). Triggers fail two ways: under-triggering misses the skill, over-triggering burns context. Without the third slot's named redirect, the agent either fires the wrong skill or fires nothing and asks the user
- **Examples are contracts** — code examples are imitated; prose requirements are interpreted. Every example carries its why. Pair anti-patterns explicitly with the correct version. If you cannot write a credible bad example, the rule probably isn't necessary
- **Banned phrases over banned behaviors** — a banned phrase ("never write 'You're absolutely right!'") is a deterministic self-check; a banned behavior ("never patronize") drifts under interpretation. Convert abstract rules into phrase-level prohibitions whenever you can
- **Tier the gate to the failure cost** — four enforcement tiers in order of teeth: prose ("should"), good/bad example + banned phrase ("usually don't"), skill with explicit triggers ("here's how"), deterministic hook ("must not"). Pick the tier whose teeth match the cost — instructions alone fail under adversarial conditions
- **Test the rule, not your intent** — strictness ladder: write supportive, neutral, and competing versions of the user's prompt; the rule must fire correctly on all three. Cold-start re-read: in a fresh session with no prior context, can the rule name the default it overrides, the failure mode it prevents, and at least one concrete trigger or example? If any of the three is missing, the rule will not fire under the conditions it was written for
- **Code over prose** — when an instruction must hold every invocation and can be encoded in a hook, schema, or `allowed-tools` restriction, encode it there. Prose enforcement degrades under context pressure; deterministic gates do not. Keep the rule documented in SKILL.md; put the teeth in code
- **Don't instruct what hooks already prevent** — if a hook deterministically blocks an action, a prose rule banning the same action is dead weight. The hook is the rule; redundant rules load on every turn while the hook does the work
- **Scope rules to the agent that runs them** — a rule that fires only in a specialized workflow (autonomous mode, subagent dispatch, a specific skill) belongs in that workflow's prompt, not the always-loaded main agent. If the failure shape is narrow, the rule is narrow
- **Long rules teach long answers** — the agent mirrors the voice of the rules it reads. A six-sentence rule banning verbose replies makes verbose replies more likely. Compress rules to one to three sentences with a good/bad example and a banned phrase. If you can't, the rule isn't ready
- **No hinging** — write the strict rule and stop. Don't pre-cover edge cases with "except when X", "unless Y", "if required Z". A correction of past agent behavior is a rule in itself, not a conditional. Agents are smart and handle exceptions on their own; pre-covering them dilutes the rule and trains the agent to hunt for opt-outs. Bad: "No ALL_CAPS (except PHP `define()` and constants)". Good: "No ALL_CAPS" — the agent keeps `define()` capitalized because the language requires it
- **Loaded cost is recurring** — SKILL.md, agent descriptions, MCP tool schemas, and Claude.md ancestors load into context on every relevant invocation, not once per session. MCP schemas cost ~500 tokens each; a 30-tool MCP server outweighs all your skills combined. Audit the per-invocation bill, not just file size. MCP and bloated agent descriptions are the biggest levers
- **Negative instructions provoke the failure they ban** — "Do NOT create new documentation files" loads "create documentation files" into active context and increases the chance the agent produces it. Replace with a positive instruction (`Edit only files under src/`) or a separate cleanup pass that removes the unwanted output after the fact
- **Self-debug before retry** — when an instruction, hook, or tool call fails repeatedly, the next action is never another attempt at the same shape. Capture what was attempted and what came back, name the pattern (loop / context overflow / state drift / wrong hypothesis), then take the smallest action that changes the diagnosis surface. The default failure-recovery is to retry until budget is gone
- **Layered diagnosis before editing prose** — when an agent misbehaves, the cause can sit in the system prompt, session history, memory, tool selection, tool execution, output rendering, or persisted state. Diagnose by ruling layers out, not by adding rules to the prompt. Rules added at the wrong layer compound bloat without fixing the failure
- **Context is finite** — every token loaded biases the agent's output. More context doesn't mean better output — irrelevant content dilutes the signal and steers the agent toward wrong concerns. Everything loaded is necessary or harmful
- **No execution narrative** — docs state what *is*, never the story of how the code got there (what was migrated, moved, or tried this session). Git owns the journey. Bad: "agents were moved from `claude/` to `agents/`". Full treatment in [claude-md.md](references/claude-md.md)
- **Progressive disclosure** — main doc holds the complete core job; deeper tiers are opt-in. Three tiers: entry points (routing + principles) → topic files (one complete workflow) → deep dives (specs, examples, edge cases)
- **Agents skip references** — a reference is an optional read; nothing forces the agent to open it, and an overconfident agent acts on the main doc without opening it. So the main doc (SKILL.md, agent body) carries everything the agent must have to get the output right; a reference holds only the step-by-step procedure for one specific hard action, opened when the agent commits to that action. Test: if skipping a reference changes the output, that content was in the wrong file — move it up
- **Split along tasks, not topics** — will different tasks need different parts? Split. Will every task need everything? Don't
- **One job per file** — focused files > fewer files
- **Trace actual flows** — follow how agents use skills to find gaps
- **Direct language** — "Use X" not "consider using X". Never: consider, might, should, could, maybe, perhaps
- **Signs of bloat** — decorative formatting, examples repeating what the rule said, process sections duplicated across files, tables instead of bullets

## Triggers

- "cc", "claude code", "claude-code"
- Creating/editing skills, agents, hooks, plugins
- Updating Claude.md documentation
- Context engineering and efficiency
- Testing skills with subagents
- Sharing skills upstream

## Topics

Read the reference that matches the problem you're solving:

- Teach agents a reusable process → [building-skills.md](references/building-skills.md)
- Verify a skill works under pressure → [testing-skills.md](references/testing-skills.md)
- Document project context for agents → [claude-md.md](references/claude-md.md)
- Automate reactions to events → [automating-with-hooks.md](references/automating-with-hooks.md)
- Create a specialized agent → [writing-agents.md](references/writing-agents.md)
- Share setup with other projects → [plugins-marketplace.md](references/plugins-marketplace.md)
- Find a built-in command → [user-commands.md](references/user-commands.md)
- Update this skill for a new release → [updating-cc-skill.md](references/updating-cc-skill.md)

## Quick Reference

### Skill Locations

- **Personal:** `~/.claude/skills/skill-name/`
- **Project:** `.claude/skills/skill-name/`
- **Plugin:** `plugin/skills/skill-name/`

### Agent-Only Skills

Skills with empty description frontmatter won't appear in available_skills context but can still be loaded by agents via the `skills:` frontmatter field. See [writing-agents.md](references/writing-agents.md) for the full pattern.

### File Naming

- `SKILL.md` (uppercase, not Skill.md) — Codex's loader only discovers the uppercase name; Claude Code finds it either way
- `Claude.md` (PascalCase, not CLAUDE.md)

### Rules

Modular project instructions in `.claude/rules/`. Alternative to Claude.md for scoped guidance.

- **User:** `~/.claude/rules/` — all projects
- **Project:** `.claude/rules/` — current project (higher priority)
- Files discovered recursively. Symlinks supported
- `paths:` frontmatter scopes rules to file globs (e.g. `paths: **/*.ts` or a YAML list of globs). Without it, rules apply unconditionally
- Load order: user rules → project rules → Claude.md hierarchy (all coexist)
- One topic per file, descriptive filenames, subdirectories to organize

### Style

- Use bullets, not tables. Tables waste tokens on formatting.

## Process

1. **Identify topic** from triggers above
2. **Read relevant reference** for detailed guidance
3. **Follow reference instructions** exactly
