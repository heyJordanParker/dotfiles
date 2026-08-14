# Writing Agents

The Process for writing `agents/<agent>.md`: frontmatter config, one Frame, and that Frame's Principles.

## 1. Put only agent-owned content in the file

- An agent file owns the frontmatter description, config frontmatter, one Frame, and the Frame's Principles.
- Rules live in Rule Files.
- A Process lives in a Skill, and an agent names that Skill with `skills:`.
- `scripts/agents.py` inlines named Skills into generated Codex artifacts.
- The body becomes the Agent's system Prompt; the Agent does not receive the full Claude Code Prompt.

IF the Prompt needs ordered steps:
### Move the Process to a Skill
A Process belongs in `skills/<skill>/SKILL.md`; the agent names it with `skills:`.

IF the Prompt needs tactical corrections:
### Move Rules to a Rule File
Rules belong in `.claude/rules/` or the package Rule File that owns their Condition, not in an agent.

IF the work is a recurring Frame that needs different Principles, tools, model, or permissions:
### Create an agent
Use an agent when the Task recurs across sessions and the separate Frame or runtime config is the thing that makes it work.

## 2. Write the frontmatter

- `name` is required, lowercase with hyphens, and matches the filename.
- `description` is required and is the trigger Claude Code uses to auto-select the agent.
- `tools` allows only the listed tools; omitting it inherits all tools.
- `disallowedTools` denies listed tools.
- `model` accepts `opus`, `sonnet`, `haiku`, `fable`, a full model identifier, or `inherit`; `inherit` matches the spawning conversation and is the default when omitted.
- `best`, `opusplan`, and `[1m]` long-context model variants resolve.
- `effort` takes `low`, `medium`, `high`, `xhigh`, or `max`, and one declaration governs both Harnesses. Claude also accepts an integer; that fails a codex run, so never write one.
- `skills` injects Skills into the Agent Context at startup when the agent is dispatched as a Subagent.
- `permissionMode` accepts `acceptEdits`, `auto`, `bypassPermissions`, `default`, `dontAsk`, or `plan`.
- `color` sets the user interface accent color; invalid values are dropped.
- `maxTurns` limits agentic turns.
- `initialPrompt` auto-submits a first turn when the agent starts.
- `mcpServers` scopes Model Context Protocol servers to the agent.
- `hooks` scopes Hook wiring to the agent.
- `memory: none` is ours, not the Harness's: it denies the Agent Memory. The key is optional and has no other value — the Harness's `user`, `project`, and `local` are off in every root (step 6).
- `readonly: true` is ours, not the Harness's: it takes writing away from the Agent on both Harnesses — the write tools, the shell commands that change the tree, and output redirection. The key is optional, has no other value, and omitting it leaves the Agent writing (step 7).
- `mode: orchestrate|build|interview` is ours, not the Harness's: `orchestrate` spawns Subagents and mutates nothing, while read-only commands still run; `build` writes and spawns nothing; `interview` neither writes nor spawns. Every roster Agent declares it, so the `build` fallback an unreadable declaration falls to never decides a real dispatch. It gates a dispatched Agent alone; on a main session the mode only picks which Skill loads (step 8).
- `ssh: enabled` is ours, not the Harness's: it lets the Agent reach another machine. It is the one opt-in declaration, so omitting it denies (step 7).
- `codex-model` is ours, not the Harness's: the model the Agent runs on under codex, where `model` names a Claude model and reaches nothing.
- `harness` is ours, not the Harness's: `all`, `claude`, or `codex`, naming where the Agent may run. It is optional and means `all` when absent. A `harness: codex` Agent declares `codex-model` and no `model`.
- `background: true` always runs the agent in the background.
- `isolation: worktree` runs the agent in a temporary git worktree.
- `isolation: remote` runs the agent in a remote Claude Code remote environment and always backgrounds it.
- `worktree.sparsePaths` limits large worktrees to selected paths.
- `worktree.baseRef` accepts `fresh` or `head`; `fresh` branches from `origin/<default>`, while `head` carries local unpushed commits into the worktree (v2.1.133+).

IF an Agent should run on a different codex model:
### Declare `codex-model` only on Evidence covering the Agent's whole job
The key is per Agent, so every Skill it runs moves with it. Evidence from one Skill is not Evidence for the Agent.
Never: moving an Agent to a cheaper model because one of its Skills scored well on it.

### Name the model category, never a version or a variant
Every place a Claude model is named — agent frontmatter `model`, settings JSON `"model"`, a `--model` flag — takes the category: `opus`, `sonnet`, `haiku`, `fable`. A pinned version or a variant suffix goes stale on the next release and has to be hunted down everywhere it was written. The category already resolves to the long-context variant: a bare `opus` session reports a 1,000,000-token context window, while a pinned `claude-opus-4-5` reports 200,000.
Never: `opus[1m]`, `claude-fable-5[1m]`, `claude-opus-4-5`.

### Keep `fallbackModel` out of agent frontmatter
The agent `.md` parser ignores `fallbackModel` in frontmatter (v2.1.195). Set fallback models in settings JSON or with `--fallback-model`.

### Use `description` as the complete trigger
The frontmatter description is the only trigger surface. The body explains the Frame and Principles after the agent is already selected.
Example: `description: Use for read-only external research with sources. DO NOT TRIGGER for in-codebase tracing; use explorer.`
Never: a trigger section in the body.

Template:
  ```yaml
  ---
  name: <agent-name>
  description: |
    Use when <the Task and trigger phrases>.
    DO NOT TRIGGER for <adjacent Task>; use <other Skill or agent>.
  color: <terminal color>
  model: inherit
  tools: Read, Grep, Glob, Bash
  skills: <process-skill>
  ---

  You are <Frame>.

  # Principles

  ## <Principle>

  <How this Frame decides when the choice is unclear.>
  ```

Example:
  ```yaml
  ---
  name: researcher
  description: |
    Use for external research with sources — library docs, APIs, framework references, web lookups,
    and vendor specs. DO NOT TRIGGER for in-codebase Architecture tracing; use explorer.
  color: green
  model: opus
  tools: Read, Grep, Glob, Bash, WebFetch, WebSearch
  skills: trace
  ---

  You are a researcher. You investigate external systems and return verified findings with sources.
  You never write code or modify files.

  # Principles

  ## Sources decide

  Prefer official documentation and source code. When sources conflict, keep reading until the
  conflict resolves.
  ```

## 3. Use agents from Claude Code

- `claude --agent <name>` starts the session with that agent as the main thread.
- The agent's system Prompt replaces the default Claude Code system Prompt for `claude --agent <name>`.
- Tool restrictions, model, permission mode, `hooks:`, and `mcpServers:` apply to `claude --agent <name>`.
- The selected agent persists on resume and the header shows `@<name>`.
- `--print` honors an agent's `tools:` and `disallowedTools:` fields (v2.1.119+).
- Plugin-contributed agents resolve without the `plugin:` prefix (v2.1.143+).
- `--agent <plugin>:<agent>` selects an agent from a specific plugin.
- `"agent": "name"` in settings JSON sets the default agent for sessions; the command-line flag overrides it.
- `--agents <json>` defines ephemeral Subagents for the session.
- `.claude/agents/` is the project location and should be committed.
- `~/.claude/agents/` is the user location.
- Plugin `agents/` directories apply where the plugin is enabled.
- `--agents` has the highest priority on name collision, then `.claude/agents/`, then `~/.claude/agents/`, then plugin `agents/`.

### Do not confuse `--agent` and `--agents`
`--agent` sets the main Agent for the session. `--agents` defines ephemeral Subagents available inside the session.

## 4. Dispatch Subagents correctly

- Built-in Task tool `subagent_type` values include `Explore`, `Plan`, `general-purpose`, and `Bash`.
- `Explore` uses Haiku and is read-only.
- `Plan` inherits the model and is read-only.
- `general-purpose` inherits the model and has all tools.
- `Bash` inherits the model and runs terminal commands in separate Context.
- Custom agents in `.claude/agents/` or `~/.claude/agents/` are also available as Subagent types.
- `subagent_type` matching is case-insensitive and separator-insensitive; `"Code Reviewer"` resolves to `code-reviewer` (v2.1.140+).
- Subagents can spawn Subagents up to five levels deep (v2.1.172+).
- Permission Rules can constrain spawns with `Agent(type)` deny Rules, `Agent(x,y)` allowed types, and `Tool(param:value)` matches such as `Agent(model:opus)` (v2.1.178+/v2.1.186+).
- Every dispatch is async: the call returns an agentId at once and the report arrives later in a completion notification.
- Agent teams are experimental behind `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`; `TeamCreate` and `TeamDelete` are removed (v2.1.178+), every session has one implicit team, and `team_name` is accepted but ignored.

### Continue a previously spawned agent with `SendMessage({to: agentId})`
The Agent tool no longer accepts a `resume` parameter. Use `SendMessage({to: agentId})` to continue a previously spawned agent.

### Let `SendMessage` resume stopped agents
`SendMessage` auto-resumes stopped agents in the background without an error.

### Do not relay permission requests through `SendMessage` from another Claude session
`SendMessage` relayed from another Claude session carries no user authority, so the receiver refuses relayed permission requests (v2.1.166+).

### Read the background task output file path
`TaskOutput` is deprecated. Use `Read` on the background task's output file path.

### Never pass the Agent tool's `name` parameter
`name` makes the dispatch an `in_process_teammate`, whose final text is never returned to the dispatcher — its only channel back is `SendMessage`, which the Subagent must look up before it can call.
/delegate owns the dispatch mechanics.

## 5. Preload Skills only when they are visible

- A Subagent does not carry `references/` or run scripts unless a preloaded Skill gives it those.
- A preloaded Skill makes its `references/` available and runs its `!` commands at load with no tool call and no Agent tokens.
- Skill preload fires on Subagent dispatch through the Task or Agent tool.
- Running an agent as the main session with `--agent` does not preload its `skills:` bodies.

### Preload resolves against the visible Skill listing
An agent's `skills:` preload matches names only against the model-invocable `available_skills` set, the same listing the main Agent sees. Hide a Skill and the loader warns `Skill '<name>' specified in frontmatter was not found`, then skips it.

### Do not try to hide a Skill from the main Agent while preloading it into one agent
`disable-model-invocation: true` and `paths:` hide a Skill from the visible listing, so both also make it non-preloadable.

### Do not leave `description:` empty to hide a Skill
An empty Skill description falls back to the Skill body in `available_skills`, leaking the whole body into every session.

### Accept that hidden-and-preloaded is not a supported shape
No frontmatter combination makes a Skill both hidden from the main Agent and injectable into a Subagent.

Template:
  ```yaml
  ---
  name: code-reviewer
  skills: review
  ---
  ```

Example:
  ```yaml
  ---
  name: code-reviewer
  description: Use for code quality Review. DO NOT TRIGGER for Architecture Review; use architect.
  skills: review
  ---
  ```

## 6. Decide whether the Agent needs Memory

- Needs it: write no `memory` key at all.
- Does not need it: write `memory: none`, which denies the Agent Memory on both Harnesses. The value is ours, not the Harness's.

IF the agent is a one-shot execution agent:
### Declare `memory: none`
A conclusion drawn weeks ago sidetracks a one-shot Task, and a write from one pollutes Memory for every Agent after it. Declare it and give the run everything it needs in the Prompt.

## 7. Decide what the Agent may reach

- Writes code: write no `readonly` key.
- Reads and reports: write `readonly: true`.
- Reaches another machine: write `ssh: enabled`. Absent, it reaches none.

IF the Agent's Frame says it reports findings and never writes:
### Declare `readonly: true`
The Prompt claims it; the declaration keeps it. `block_denied_access.py` then refuses the write tools, the shell commands that change the tree, and output redirection, on both Harnesses.
Never: relying on `tools:` alone, which Claude honours and codex drops, and which leaves the shell open on both.

IF the Agent must run the suite, a build, or any package manager:
### Leave `readonly` off
A read-only Agent runs no runtime and no package manager, because each one runs whatever it is handed. An Agent that must execute code is not read-only, and forcing the key on it costs the capability instead of protecting anything.

IF a read-only Agent needs a command the guard refuses:
### Add the command to `_READERS`, never a second declaration
The allowlist in `block_denied_access.py` is the one place that decides. A refusal names the command it blocked, so the Agent's report says which word to add.

## 8. Decide how the Agent works

- Does the work itself: write `mode: build`, and name `build` in `skills:`.
- Hands every Task to Subagents: write `mode: orchestrate`, and name `delegate` and `orchestrate` in `skills:`.
- Interviews the Architect, neither writes nor spawns: write `mode: interview`.

### Declare `mode` on every Agent
The key is written on every roster Agent, `orchestrate`, `build`, or `interview`, so the gate reads the author's intent rather than a fallback. `build` is what `session_mode.py` falls to when it cannot read a declaration at all, not an authoring shortcut.
Never: leaving `mode` off and relying on the fallback.
