# WHY

Prompt building-block workspace where each file type owns exactly one kind of Prompt block: Frame, Skill Process, Command, Hook, or Agent tooling.

# Facts

- `agents/<agent>.md` carries frontmatter, one Frame, and Principles.
- One `agents/<agent>.md` is the whole Agent on both Harnesses: dispatched as a Claude Subagent by that name, and run on codex as `codex-run @<name>`. This holds for a profile's own Agents under `packages/claude/profiles/<name>/agents/` too.
- `codex-run` resolves `@<name>` against the active config root's `agents/` first and `~/.agents/agents` second, so a profile adds its roster to the shared one and a name held by both runs as the profile's.
- `scripts/sync.py` generates the codex artifacts for every profile's own Agents as well as this roster, skipping a symlinked `agents/` or `<agent>.md` because those generate where they really live.
- The copywriter profile reaches `designer.md` and `context-engineer.md` by symlink into this roster.
- The Harness's own per-Agent memory (`memory: user|project|local`, `~/.claude/agent-memory/<name>/`) is switched off in the default root by `autoMemoryEnabled: false` and live under every profile root, which does not set the key.
- `memory: none` in an Agent's frontmatter denies it Memory on a Claude Subagent dispatch and on a `codex-run` run, founding or resumed; omitting the key leaves Memory reachable.
- `block_memory_access.py` enforces `memory: none` on Claude by blocking the honcho tools; `lib/codex_run.py` enforces it on codex by switching off both of codex's memory providers, the honcho server and codex's own `[memories]`.
- Both enforcement points read the declaration through `lib/agent_memory.py`, which honours quoted and commented spellings and treats an unreadable definition as a denial.
- Every generated `<name>.prompt.md` opens with `<!-- codex-run agent: <name> -->`, which carries the Agent's identity into codex's record of the run so rewording the Agent's Prompt never orphans a thread founded under it.
- `codex-run resume` recovers the founding Agent from codex's own rollout for the thread: the name in the recorded `base_instructions` marker when there is one, resolved through the roster so a stale or tampered marker names nothing, and otherwise a match of the recorded text against `<name>.prompt.md` whole first and on the first line second, for threads founded before the marker existed. It then runs with that Agent's instructions and declaration; a thread it cannot identify fails the run instead of continuing as codex's default Agent.
- `commands/<command>.md` is a Command the Architect invokes manually.
- `skills/<skill>/SKILL.md` is a Skill manifest and its Process.
- `skills/<skill>/references/<process>.md` is a Reference for a Process split out for Progressive Disclosure.
- `hooks/<module>.py` is the shared Python Hook source.
- `tooling/<name>/` is a buildable kit invoked by Agents through `~/.agents/tooling/<name>/`.
- `scripts/agents.py` generates Codex Agent artifacts from `agents/<agent>.md`.
- `scripts/hooks.py` generates Hook wiring from each Hook's `BINDING`.
- A `BINDING` declaring `roots: "all"` is generated into every profile's `settings.json` as well as the default root's.
- The `/cc` Skill is the Process for writing Prompts.
