# WHY

Prompt building-block workspace where each file type owns exactly one kind of Prompt block: Frame, Skill Process, Command, or Hook.

# Facts

- `agents/<agent>.md` carries frontmatter, one Frame, and Principles.
- One `agents/<agent>.md` is the whole Agent, dispatched as a Claude Subagent by that name. This holds for a profile's own Agents under `packages/claude/profiles/<name>/agents/` too.
- `readonly: true` in an Agent's frontmatter takes writing away from it on both Harnesses: the write tools, every shell command that changes the tree, and output redirection. Omitting the key leaves it writing. The rulebook rule `block-writes-by-readonly-agents` enforces it, and an unreadable definition denies.
- `mode: orchestrate|build` in an Agent's frontmatter names how it works in a session the Architect starts on it: `orchestrate` spawns Subagents and `build` spawns none. The rulebook rule `block-spawning-in-build-mode` enforces it.
- A Claude Subagent starts other agents only when its own definition declares `mode: orchestrate`, such as a project manager running its team. Every other Subagent builds, and the rulebook rule `block-spawning-by-build-subagents` refuses its launches. `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=2` caps nesting at the architect's session, its Subagent, and that Subagent's team. A call is a Subagent's only when it carries `agent_id`, which Claude sends only inside a Subagent call. The architect's sessions started with `--agent` carry `agent_type` without `agent_id`, so they stay his.
- `ssh: enabled` is the one opt-in declaration: absent, an Agent cannot reach another machine (`ssh`, `scp`, `sftp`, `rsync`, `mosh`, `telnet`), because the architect is the only one who touches production. It is independent of `readonly`, so an Agent can read a machine and change nothing.
- A command carried inside another one, such as a shell's `-c` string or an `ssh` line, reaches the rulebook rules by name, because `parseShell` of `@cmodjs/core` reads it.
- `tools:` is the Harness's own field, honoured natively by Claude and not by codex, so `block-writes-by-readonly-agents` also refuses the write tools on codex to an Agent whose `tools:` names none. Claude reaches the same answer by never offering them. An Agent with no `tools:` line has every tool on both Harnesses.
- An agent codex spawns is gated by the role codex names on the payload as `agent_type`, and a role with no definition, such as codex's own `default` and `worker`, runs as `build`. An interactive codex session names no Agent and is never gated by a declaration. Our roster is not published as codex roles: a role backed by a `config_file` fails to apply in codex 0.153.4, so a role file would govern nothing and would shadow codex's own.
- References live in `skills/<skill>/references/<process>.md`.
- `scripts/hooks.py` generates Hook wiring from each Hook's `BINDING`.
- A Hook's injected context stays under `lib/feedback.py`'s `CONTEXT_LIMIT`, 10,000 characters, because Claude Code saves a longer hook message to a file and shows the agent its path and a 2,000-character preview.
- The `/cc` Skill is the Process for writing Prompts.
