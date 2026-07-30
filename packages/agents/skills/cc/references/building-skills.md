# Skill

The Process for writing or fixing a Skill: one ordered Process that corrects a recurring
Agent failure. A Skill never carries a Frame, never carries broad Principles, and never
carries WHY; those live in the Agent and Claude.md.

## 1. Name the gap

State the Agent's default and the behavior wanted instead. No observed gap, no Skill.

### Watch the Agent fail before writing the correction
A line that corrects no failure is Fluff. The control run in testing-skills.md decides when
unsure.

## 2. Find the Precedent

Read the sibling Skills closest to the Task and match their shape. Check for overlap before
creating anything.

- The source of truth is `packages/agents/skills/<name>/SKILL.md`.
- Stow and plugin packaging copy the source into the runtime locations.
- Runtime locations include `~/.claude/skills/`, `.claude/skills/`, and plugin `skills/`.

### Extend the overlapping Skill instead of creating a near-duplicate
Similarity is a bug. A near-duplicate is the existing Skill's missing Rule, Example, Template,
Condition, or Reference.

### Model a new Process on the real human team that already does this work
Thousands of years of human process optimization beat invention. When no repo Precedent
exists, the Precedent is the real-world team: name the human role or process the Skill
or Agent roster mirrors (editorial desk, agency, code review) before shaping anything.
Never: an invented roster or Process no working human team has ever run.

### Read every file before editing an existing Skill
Read SKILL.md and every Reference in full first. Piecemeal edits create contradictions.

## 3. Write the frontmatter gate

A Skill needs `name` and `description`. `name` is lowercase hyphenated text up to 64 characters and
matches the directory. `description` is up to 1024 characters and is never empty: an empty
`description` leaks the body into listings. Listings show at most 1536 characters of a
description.

### Make the description the only trigger
The body never carries a trigger section. Write the description as what the Skill does, TRIGGER
phrases Users actually say, and DO NOT TRIGGER with the adjacent case plus the Skill that fires
instead.
Example: `description: Write and fix Claude Code Prompts. TRIGGER when the task says "cc" or asks to build a Skill. DO NOT TRIGGER to name code identifiers; use /naming.`
Never: `description: Use this when needed.`

### Add frontmatter keys only when the Skill reaches for them
Common keys: `allowed-tools`, `disallowed-tools`, `context: fork`, `agent`, `model`, `effort`,
`hooks`, and `argument-hint`. `disable-model-invocation: true` makes the Skill user-only and drops
it from the listing; `user-invocable: false` hides it from the slash menu only.

### Keep side effects user-only
A Process that edits code, runs a pipeline, or starts a server is the Architect's to fire. That is
a Command.

### Read the live schema for unknown keys
For any frontmatter key not named here, use updating-cc-skill.md instead of a memorized catalog.
No XML angle brackets in frontmatter. No `claude` or `anthropic` in Skill names.

## 4. Write SKILL.md as the whole method

Everything needed 80 percent of the time lives in SKILL.md. An overconfident Agent acts from
SKILL.md alone, so it must carry the whole Process.

### Use steps only when order matters
A Skill is a Process, so numbered steps are normal. Each step carries only the Rules, Facts,
Examples, Templates, Conditions, and Verification that correct that step.

### Put live state in front of the Agent with an auto-run line
A backtick-wrapped command prefixed with `!` runs at expansion and inlines stdout before the
Agent's Bash tool and guard Hooks.
Template:
  ````markdown
  ## Current Changes

  !`git changes`
  ````

`${CLAUDE_EFFORT}` resolves to the active effort level. In command bodies, escape a literal `$`
before a digit as `\$`. The `disableSkillShellExecution` setting turns inline `!` execution off.

### Derive volatile lists live
Enumerating files or dependencies inline goes stale. Have the Skill derive them live with repo
commands or trace.
Example: `git ls-files packages/agents/skills/cc/references`.
Never: a copied list of every Reference file.

## 5. Split a Reference only for a hard sub-task

A Reference is a Process split out for Progressive Disclosure. Split only a hard sub-task the
Agent needs under 80 percent of the time. Never split a roster, catalog, worked-example set, or
background reading.

### Name the Reference by the problem it solves
The link line names the doing, not the component.
Example: `Your Example is not changing behavior → building-examples.md`.
Never: `hooks.md — hook system`.

## 6. Fill the Template

Template:
  ```markdown
  ---
  name: skill-directory-name
  description: What this Skill does. TRIGGER when Users say the real phrases. DO NOT TRIGGER when the adjacent case applies; use other-skill.
  ---

  # Skill Name

  - Fact the Agent needs before running the Process.

  ## 1. First ordered step
  What the Agent does in this step.

  IF observable Condition:
  ### Rule title written as the action to take
  Explanation of the correction.
  Example: correct behavior the Agent can imitate.
  Never: wrong behavior paired to the correction.

  Template:
    fill-the-blanks Example for the output shape

  ## References (each solves one problem)

  - Problem this Reference solves → reference-file.md
  ```

Example:
  ```markdown
  ---
  name: cc
  description: Write and fix Claude Code Prompts — skills, agents, commands, hooks, rules files, Claude.md, and plugin distribution. TRIGGER when the task says "cc" or asks to build, edit, test, or share a Skill. DO NOT TRIGGER to name code identifiers; use /naming.
  ---

  # Claude Code

  - Every file under `packages/agents/` is a Prompt.

  ## 1. Name the Prompt type and fill only what it owns

  ### Move a misplaced block to its home first
  A block in the wrong file type invalidates every later step.

  ## References (each solves one problem)

  - Building or restructuring a Skill → building-skills.md
  ```

Never: a body trigger section, a Frame, broad Principles, a roster split into a Reference, or a
blank Rule slot filled for symmetry.

## 7. Verify against real behavior

Testing-skills.md is the Process. Done means the Agent produces the right output from SKILL.md
alone, every line names a default it overrides, and the description fires on the real phrases but
not the adjacent ones.
