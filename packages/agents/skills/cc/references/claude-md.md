# Claude.md

The Process for documenting a folder: its WHY and the Facts its whole folder consumes.

## 1. Write the WHY

One item: the business problem and the Architect's thinking behind the folder's shape.

### Use domain language, never library names
"Domain models never import plugin code" outlives "Models never import FunnelKit."

## 2. Collect the Facts

Plain sentences, one per line. A Fact earns its line by sparing the Agent research the
folder's work would otherwise force: the command that runs this folder, behavior confirmed
empirically, where a record or Skill lives.

### Keep only Facts the whole folder consumes
A Fact one Process consumes goes on that Skill's step, not here.

## 3. Push it to the deepest folder it applies to

A deep file inherits every Claude.md above it.

### Never repeat a parent Claude.md
The chain loads together, so duplication reads as emphasis while doubling the staleness
risk. Litmus: would an Agent in a sibling folder need this line? No: it stays deep.

### Write what is, never what changed
Git owns the journey. A line that reads as something that changed is cut.
Example: "Agents source of truth: `packages/agents/agents/<name>.md`."
Never: "Agents were moved from `packages/claude/agents/` to `packages/agents/agents/`."

## 4. Fill the Template

Template:
  # WHY

  One clone rebuilds the whole machine: GNU Stow lays every package at its target,
  and the claude package doubles as the plugin marketplace source, so the tree that
  runs this machine is the tree that ships to consumers.

  # Facts

  - The domain's words live in `/Domain.md`.
  - The Process for writing Prompts is the /cc Skill.
  - This project uses Laravel and React + TanStack Query.

Never: a file tree (tracer shows structure at runtime), Rules (rules files), a Process
(Skills), vocabulary (Domain.md), or Decisions inline (docs/architecture/decisions/).
