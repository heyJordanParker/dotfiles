# WHY

Prompt building-block workspace where each file type owns exactly one kind of Prompt block: Frame, Skill Process, Command, or Hook.

# Facts

- `agents/<agent>.md` carries frontmatter, one Frame, and Principles.
- One `agents/<agent>.md` is the whole Agent on both Harnesses: dispatched as a Claude Subagent by that name, and run on codex as `codex-run @<name>`. This holds for a profile's own Agents under `packages/claude/profiles/<name>/agents/` too.
- `codex-run` resolves `@<name>` against the active config root's `agents/` first and `~/.agents/agents` second, so a profile adds its roster to the shared one and a name held by both runs as the profile's.
- `scripts/sync.py` generates the codex artifacts for every profile's own Agents as well as this roster, skipping a symlinked `agents/` or `<agent>.md` because those generate where they really live.
- The copywriter profile reaches `designer.md` and `context-engineer.md` by symlink into this roster.
- The Harness's own per-Agent memory (`memory: user|project|local`, `~/.claude/agent-memory/<name>/`) is switched off by `autoMemoryEnabled: false` in every root, the default one and each profile.
- `memory: none` in an Agent's frontmatter denies it Memory on a Claude Subagent dispatch and on a `codex-run` run, founding or resumed; omitting the key leaves Memory reachable.
- Memory is Honcho, reached through the `honcho` command in `packages/bin/`, which wraps `lib/honcho.py` the way `codex-run` wraps `lib/codex_run.py`. There is no memory MCP server on either Harness.
- Memory is stored one peer per human and one per agent, all inside one session per repository, and no peer observes another: each peer's collection holds Honcho's conclusions about that peer's own messages, so nothing is derived twice.
- `remember_architect_message.py` and `remember_agent_message.py` write the messages and `remember_tool_use.py` writes one `[work]` line per Write/Edit/Bash/Agent call, so memory holds what an Agent did and not only its account of it; reads are excluded because `trace` records those per file.
- `inject_honcho_memory.py` reads the architect's collection and the running Agent's — unsearched at `SessionStart` and `PreCompact`, searched on the turn's own words at `UserPromptSubmit`, and skipped entirely for an acknowledgement or a slash command. The architect's block opens with his peer card, the part that does not depend on the turn matching a conclusion. Each retrieval is capped well under the write timeout and falls back to the last answer that arrived, so a slow server makes a turn stale rather than blind; an answered "nothing stored" is not a failure and clears that fallback. `lib/event.py`'s `agent_name` names the running agent from `agent_type` on Claude and from `CODEX_RUN_AGENT_FILE` on codex.
- No prompt event fires inside a Claude Subagent, so the same Hook reaches one through its dispatch: on `PreToolUse` for the `Agent` tool it prepends the dispatched Agent's own Memory and the architect's to the brief through `updatedInput`, keyed off `subagent_type` so the peer is the Agent about to run rather than the one dispatching it.
- `honcho context` returns stored conclusions, `honcho search` the messages behind them, and `honcho ask` Honcho's own reasoning over a peer in prose. A question about what was decided is a `search` or an `ask`.
- `honcho remember <text>` keeps one thing in the running Agent's own collection, and never takes an Agent name — a name is the one thing an Agent would get wrong. `CODEX_RUN_AGENT_FILE` names a codex run and `CLAUDE_CODE_AGENT` names a Claude session.
- Inside a Claude Subagent `CLAUDE_CODE_AGENT` holds the dispatching Agent, so `name_memory_caller.py` writes `--as <agent>` into the command before it runs: Claude names the Subagent on the `PreToolUse` payload, and a `PreToolUse` Hook can return `updatedInput` to replace a Bash command, verified live. It carries no `permissionDecision`, so it grants nothing another gate would refuse. A call it cannot reach — one behind an environment prefix — is refused rather than left to resolve wrongly.
- `memory: none` denies an Agent Memory in every direction: `block_memory_access.py` refuses the `honcho` command on both Harnesses, `inject_honcho_memory.py` injects nothing, `remember_agent_message.py` stores nothing, and `lib/codex_run.py` switches off codex's own `[memories]`, which injects a Memory section with no tool call at all. All read the declaration through `agent_memory.definition_path`, which resolves a codex run by its exported path and a Claude Agent under the active config root.
- Both enforcement points read the declaration through `lib/agent_memory.py`, which honours quoted and commented spellings and treats an unreadable definition as a denial.
- `codex-model` in an Agent's frontmatter names the model its `codex-run` run uses, founding or resumed; omitting it runs the default in `lib/codex_run.py`. It is read through `lib/agent_memory.py` from the same roster as `memory`.
- `tools:` is the Harness's own field, honoured natively by Claude and dropped from the codex artifacts, so `block_undeclared_tools.py` refuses `apply_patch` on codex to an Agent declaring no write tool. It denies only the write family, because every read-only Agent declares `Bash` and shell writes are reachable on Claude too. An Agent with no `tools:` line has every tool on both Harnesses.
- `lib/codex_run.py` puts the running Agent's definition path in `CODEX_RUN_AGENT_FILE`, which is how a codex-side Hook knows which Agent it is gating; an interactive codex session sets nothing and is never gated.
- `effort` is one field for both Harnesses: `low`, `medium`, `high`, `xhigh`, or `max`, read natively by Claude and passed verbatim to codex by `lib/codex_run.py`, which accepts the same five. Omitting it runs the default there; any other word fails the run.
- Every generated `<name>.prompt.md` opens with `<!-- codex-run agent: <name> -->`, which carries the Agent's identity into codex's record of the run so rewording the Agent's Prompt never orphans a thread founded under it.
- `codex-run resume` recovers the founding Agent from codex's own rollout for the thread: the name in the recorded `base_instructions` marker when there is one, resolved through the roster so a stale or tampered marker names nothing, and otherwise a match of the recorded text against `<name>.prompt.md` whole first and on the first line second, for threads founded before the marker existed. It then runs with that Agent's instructions and declaration; a thread it cannot identify fails the run instead of continuing as codex's default Agent.
- `commands/<command>.md` is a Command the Architect invokes manually.
- `skills/<skill>/SKILL.md` is a Skill manifest and its Process.
- `skills/<skill>/references/<process>.md` is a Reference for a Process split out for Progressive Disclosure.
- `hooks/<module>.py` is the shared Python Hook source.
- `scripts/agents.py` generates Codex Agent artifacts from `agents/<agent>.md`.
- `scripts/hooks.py` generates Hook wiring from each Hook's `BINDING`.
- A `BINDING` declaring `roots: "all"` is generated into every profile's `settings.json` as well as the default root's.
- The `/cc` Skill is the Process for writing Prompts.
