# WHY

Prompt building-block workspace where each file type owns exactly one kind of Prompt block: Frame, Skill Process, Command, Hook, or Agent tooling.

# Facts

- `agents/<agent>.md` carries frontmatter, one Frame, and Principles.
- `commands/<command>.md` is a Command the Architect invokes manually.
- `skills/<skill>/SKILL.md` is a Skill manifest and its Process.
- `skills/<skill>/references/<process>.md` is a Reference for a Process split out for Progressive Disclosure.
- `hooks/<module>.py` is the shared Python Hook source.
- `tooling/<name>/` is a buildable kit invoked by Agents through `~/.agents/tooling/<name>/`.
- `scripts/agents.py` generates Codex Agent artifacts from `agents/<agent>.md`.
- `scripts/hooks.py` generates Hook wiring from each Hook's `BINDING`.
- The `/cc` Skill is the Process for writing Prompts.
