# WHY

Prompt building-block workspace where each file type owns exactly one kind of Prompt block: Frame, Skill Process, Command, or Hook.

# Facts

- `agents/<agent>.md` carries frontmatter, one Frame, and Principles.
- One `agents/<agent>.md` is the whole Agent, dispatched as a Claude Subagent by that name. This holds for a profile's own Agents under `packages/claude/profiles/<name>/agents/` too.
- Interactive codex runs as the CTO: `config.toml`'s `model_instructions_file` points at `agents/cto.md`, and codex finds the Skills it names in `~/.agents/skills`.
- The Harness's own per-Agent memory (`memory: user|project|local`, `~/.claude/agent-memory/<name>/`) is switched off by `autoMemoryEnabled: false` in every root, the default one and each profile.
- `lib/agent_definition.py` reads every declaration in an Agent's frontmatter. It honours quoted and commented spellings, and its `definition_path` resolves an agent codex spawns and a Claude Agent by name in the project's `.claude/agents/` first, then under the active config root, as Claude Code does. `lib/event.py`'s `agent_name` names the running agent from `agent_type` on both Harnesses.
- `readonly: true` in an Agent's frontmatter takes writing away from it on both Harnesses: the write tools, every shell command that changes the tree, and output redirection. Omitting the key leaves it writing. The rulebook rule `block-writes-by-readonly-agents` enforces it, and an unreadable definition denies.
- `mode: orchestrate|build` in an Agent's frontmatter names how it works in a session the Architect starts on it: `orchestrate` spawns Subagents and `build` spawns none. The rulebook rule `block-spawning-in-build-mode` enforces it.
- A Claude Subagent starts other agents only when its own definition declares `mode: orchestrate`, such as a project manager running its team. Every other Subagent builds, and the rulebook rule `block-spawning-by-build-subagents` refuses its launches. `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=2` caps nesting at the architect's session, its Subagent, and that Subagent's team. A call is a Subagent's only when it carries `agent_id`, which Claude sends only inside a Subagent call. The architect's sessions started with `--agent` carry `agent_type` without `agent_id`, so they stay his.
- `classify_intent.py` names the mode Skill from `session_mode.resolve`, so the Skill an Agent loads and the mode the gates enforce come off one policy.
- A session spawns under the mode `session_mode.resolve` names: the Agent it runs on, or the mode the architect typed. A session that names no Agent and typed no mode has no chosen mode to enforce and spawns.
- Every write answers to the launching session's state axis, so a main session is held back by `propose` and let through by `execute` whatever mode it records, and a dispatch inherits its launcher's: a Subagent under a proposing session writes nothing in the tree. A dispatch with no recorded launcher runs as `execute`.
- `ssh: enabled` is the one opt-in declaration: absent, an Agent cannot reach another machine (`ssh`, `scp`, `sftp`, `rsync`, `mosh`, `telnet`), because the architect is the only one who touches production. It is independent of `readonly`, so an Agent can read a machine and change nothing.
- The shell half of both is an allowlist, not a denylist: a read-only Agent runs only what `readonlyRefusal` in the rulebook package's `lib/shell.ts` allows, and a line the rule cannot parse blocks. It allows the `READERS`, the `VALIDATORS` such as `pytest` and `tsc`, a runner such as `uv` or `npx` launching a validator, and `npm`, `pnpm`, `yarn`, or `bun` running a `CHECK_SCRIPTS` script such as `test` or `lint`. A denylist fails open on the first shape nobody listed, and `env X=1 sed -i`, `bash -c '…'`, and an absolute path are each one line. Any other runtime or package manager call is refused, because `python3`, `uv`, `npm` and `make` run whatever they are handed.
- A line whose program is not in the line is refused by the rulebook rule `block-hidden-commands`: inline runtime code (`python3 -c`, `node -e`, `-m`), a program piped into a runtime, a script file that is not our own tooling, a head the shell expands (`$SHELL`, backticks), and `alias`. `unreadable` in the rulebook package's `lib/shell.ts` decides it. A script under `/dotfiles/`, `/.agents/` or `/.claude/` is ours and runs, so `python3 scripts/sync.py` works and `sh /tmp/x.sh` does not. A runtime running its own flags names no program, so `python3 --version` reads normally.
- A command carried inside another one, such as a shell's `-c` string or an `ssh` line, reaches the rulebook rules by name, because `parseShell` of `@cmodjs/core` reads it.
- A script named in a manifest stays trusted, so `make test`, `npm run lint` and `bun run build` are read as the runner, never as the recipe behind it. A recipe can carry anything, and refusing them would refuse ordinary work in every repository.
- `tools:` is the Harness's own field, honoured natively by Claude and not by codex, so `block-writes-by-readonly-agents` also refuses the write tools on codex to an Agent whose `tools:` names none. Claude reaches the same answer by never offering them. An Agent with no `tools:` line has every tool on both Harnesses.
- An agent codex spawns is gated by the role codex names on the payload as `agent_type`, and a role with no definition, such as codex's own `default` and `worker`, runs as `build`. An interactive codex session names no Agent and is never gated by a declaration. Our roster is not published as codex roles: a role backed by a `config_file` fails to apply in codex 0.153.4, so a role file would govern nothing and would shadow codex's own.
- Commands live in `commands/<command>.md`.
- `skills/<skill>/SKILL.md` is a Skill manifest and its Process.
- References live in `skills/<skill>/references/<process>.md`.
- `hooks/<module>.py` is the shared Python Hook source.
- `scripts/hooks.py` generates Hook wiring from each Hook's `BINDING`.
- A `BINDING` declaring `roots: "all"` is generated into every profile's `settings.json` as well as the default root's.
- The Hooks bound to one event and tool run in one `hooks/combine_hooks.py` process, which feeds each the event and merges their refusals, context, and one input rewrite into one answer.
- A `BINDING` declaring `standalone: True` keeps its own process: the Hooks that inject large context or call the network or a model.
- A Hook's injected context stays under `lib/feedback.py`'s `CONTEXT_LIMIT`, 10,000 characters, because Claude Code saves a longer hook message to a file and shows the agent its path and a 2,000-character preview.
- The `/cc` Skill is the Process for writing Prompts.
