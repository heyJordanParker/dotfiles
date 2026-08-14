---
name: cc
description: Write and fix Claude Code Prompts — skills, agents, commands, hooks, rules files, Claude.md, and plugin distribution. TRIGGER when the task says "cc"/"claude code"/"claude-code", or asks to build, edit, test, or share a skill/agent/hook/command, tune a description's triggering, write or prune a Claude.md, or cut prompt bloat. DO NOT TRIGGER to name a Claude.md's identifiers (that is /naming) or to record domain vocabulary (that is /domain-design).
---

# Claude Code

- Last synced with Claude Code v2.1.195 (2026-06-28).
- Every file under `packages/agents/` is a Prompt: instructions that correct the Agent's Disposition toward what the Architect intends.
- One Process: write or fix a Prompt so every line earns its place.

## 1. Understand the problem the Prompt solves

A Prompt exists to solve one problem. Understand that problem before you touch the Prompt. Find out what the Agent does by default and what the Architect wants instead.

IF fixing an existing Prompt:
### Understand what the Prompt already solves before you change it
Every line in it was written to correct something. Work out what that is first. A change made blind to it brings the old failure back.

### Name a Disposition, never an incident
A correction you just received is one incident; a Rule corrects what the Agent does by default. Write it only when the Agent would fail the same way again without it.
Never: a Rule whose real subject is the mistake you just made.

IF unsure whether the Harness base prompt already covers the gap:
### Let the control run in step 7 decide
Never guess.

## 2. Read the Architecture and the Domain

Read `Architecture.md` beside this file. It tells you which Prompt file your change belongs in. Then read `Domain.md` beside this file for the domain language you write in.

## 3. Name the Prompt type and fill only what it owns

### Move a misplaced block to its home first
A block in the wrong file type invalidates every later step.

### Agents never carry Rules or a Process
A Process is a Skill the agent names via `skills:`; `scripts/agents.py` inlines named Skills into the codex artifacts. Rules load from rules files by glob.

### The frontmatter description is the only trigger
The body never carries a trigger section.

### A Skill step carries a block only where it corrects something
A blank filled for symmetry is Fluff. A Skill never carries a Frame, never Principles beyond its one Process.

### Only a Process becomes a Reference
A roster, catalog, worked-example set, or data table stays in SKILL.md even when that makes it long. Never split out content needed 80% of the time: the Agent writes a working Prompt from SKILL.md alone. A Reference that reads as optional background is never opened; cut it or fold it back. The link line names the problem the Reference solves.

### A Process another Skill owns is named, never restated
Use `/skill-name` in the step that needs it. Both Harnesses load the named Skill, whole.
Never name a `disable-model-invocation` Skill — the Architect fires those; an Agent's load falls through to whatever Claude ships under that name.

### A deterministic Hook IS its Rule
Duplicate prose is cut. A model-backed Hook batches its event's Rules into one call and fails open, so the prose fallback stays.

### A new file type is the Architect's Decision
The Agent proposes Decisions; only the Architect makes them, typically through /interview.

## 4. Correct with the lightest delivery that holds

Three axes pick the delivery: load guarantee (always-loaded, Condition-loaded, Skill the Agent invokes), recency (session start, per turn, at the moment of the action), strength (prose, Example, checked, blocked).

- A behavior that must hold every turn is a Hook; always-loaded prose is forgotten after a few turns.
- A step that interprets no human language is code, not Prompt.

The failure picks the form:

- Violated under pressure: a prohibition plus the red-flag phrases that precede the violation.
- Wrong output shape: a positive recipe of what the output IS. Measured twice: a banned-shape arm produced more of the banned shape, and a scope prohibition moved codex scope 0.88 to 0.75 by planting the act it named.
- Missing element: a required slot in a Template.
- Situational: a Condition on an observable predicate, as one IF line owning exactly one Rule.

Never: a nuance clause appended to a winning correction. Measured: one banned phrase plus one positive recipe moved codex communication 0.73 to 0.87, past the 11,000-word baseline; the 20-item banned-vocabulary list it replaced made replies worse.

## 5. Write every block in its shape

The shape identifies the block without its heading. Every Prompt file is flat: heading, then content; an unnumbered listicle by default, numbered when order matters, one item for the WHY. A list that has grown hard to read is broken apart into Skills, more rules files, or more folders, never subheaded.

A Fact is a plain sentence in a listicle:

  - The domain's words live in `/Domain.md`.
  - This project uses Laravel and React + TanStack Query.

A Rule is a ### title with its explanation on the lines below, no bold lead:

  ### Restow after adding, removing, or renaming any file inside a package
  Run `cd packages && stow -R -t <target> <pkg>` or `python3 scripts/sync.py`.

A Condition is one IF line above the one Rule or step it owns, markdown, never an XML tag (measured tie on firing and bleed; the shape that doesn't clash with markdown wins):

  IF adding a new package:
  ### Add its entry to `scripts/stow.py` before syncing

An Example or a Never is a labeled line directly under its Rule:

  Example: add `"ghostty"` to `CONFIG`, then `python3 scripts/sync.py`.
  Never: `stow -t ~/.config/ghostty ghostty` by hand — sync.py never restows it again.

A Template is a label with the block indented on new lines beneath it.

## 6. Audit every line

- Corrects nothing: cut.
- Corrects too hard: smaller and positive ("Use X", never "consider using X"; the Agent mirrors the voice of the rules it reads).
- Needed under 80% of the time: split out per Progressive Disclosure.
- Correcting more than the failure costs is Overprompting: the volume buries the signal and the Agent starts ignoring instructions wholesale.
- Every Domain.md word keeps its capitalization; a term that traces to neither Domain.md nor the code is coined. Consult the Architect, never write it.

## 7. Verify against real behavior, never intent

Control run first: if the failure doesn't show without the line, there is no gap and the line is not written. Then pressure-test (testing-skills.md). Done when the correction held under pressure and the control showed the gap.

## References (each solves one problem)

- Building or restructuring a skill → building-skills.md
- Your Example isn't changing behavior → building-examples.md
- Proving a skill holds under pressure → testing-skills.md
- Documenting a folder for Agents → claude-md.md
- Making the Harness react to an event → automating-with-hooks.md
- Giving one Agent a Frame → writing-agents.md
- Shipping this setup to another repo → plugins-marketplace.md
- Finding whether a built-in command already does it → user-commands.md
- Syncing this skill with a new Claude Code release → updating-cc-skill.md
