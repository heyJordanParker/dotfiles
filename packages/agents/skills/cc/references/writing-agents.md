
# Writing Agents

How to create custom subagents for Claude Code.

## When to Create

- Task type recurs across sessions (reviews, debugging, analysis)
- Need specific tool restrictions or permissions
- Want specialized system prompt for a domain
- Need to route to cheaper/faster models

## File Structure

Markdown with YAML frontmatter. Body becomes the system prompt.

```yaml
---
name: agent-name
description: When Claude should delegate to this agent
tools: Read, Grep, Glob, Bash
model: sonnet
---

System prompt here. Agent sees only this — not the full Claude Code prompt.
```

## Using Agents

- `claude --agent <name>` — Start session with agent as the main thread. Agent's system prompt **replaces** the default Claude Code system prompt. Tool restrictions, model, and permission mode apply. Persists on resume. Header shows `@<name>`. The agent's `hooks:` (v2.1.116+) and `mcpServers:` (v2.1.117+) frontmatter also load on the main thread, and `--print` honors its `tools:` / `disallowedTools:` (v2.1.119+). Resolves plugin-contributed agents without the `plugin:` prefix (v2.1.143+)
- `--agent <plugin>:<agent>` — Use agent from a specific plugin
- `"agent": "name"` in settings.json — Set default agent for all sessions. CLI flag overrides
- `--agents <json>` (plural) — Define ephemeral subagents for the session. Different from `--agent` (singular) which sets the main agent

## Locations

Priority order (highest wins on name collision):

1. `--agents` CLI flag — session only
2. `.claude/agents/` — project, check into VCS
3. `~/.claude/agents/` — user, all projects
4. Plugin `agents/` — where plugin is enabled

## Frontmatter

**Required:**
- `name` — lowercase + hyphens, matches filename
- `description` — when to delegate. Claude uses this to auto-select

**Common:**
- `tools` — tool allowlist. Inherits all if omitted
- `disallowedTools` — tool denylist
- `model` — `opus`, `sonnet`, `haiku`, `fable`, a full model ID, or `inherit` (match the spawning conversation; the default when omitted). `best`, `opusplan`, and the `[1m]` long-context variants also resolve
- `effort` — thinking effort: `low`, `medium`, `high`, `max`, or an integer
- `skills` — skills injected into agent context at startup
- `permissionMode` — one of `acceptEdits`, `auto`, `bypassPermissions`, `default`, `dontAsk`, `plan`
- `color` — accent color in the UI. One of the standard terminal colors (`red`, `cyan`, `magenta`, …); invalid values are dropped

**Advanced:**
- `maxTurns` — max agentic turns
- `initialPrompt` — auto-submit a first turn when agent starts
- `mcpServers` — MCP servers for this agent
- `hooks` — lifecycle hooks scoped to this agent
- `memory` — persistent memory: `user`, `project`, or `local`
- `background` — `true` to always run in background. Killing a background agent preserves partial results in conversation context
- `isolation` — `worktree` runs the agent in a temporary git worktree; `remote` runs it in a remote CCR environment (always backgrounded). Use `worktree.sparsePaths` setting in large monorepos to check out only specific directories. The `worktree.baseRef` setting (`fresh` | `head`) chooses the branch base: `fresh` (default) branches from `origin/<default>`, `head` carries local unpushed commits into the worktree (v2.1.133+)

**Not agent frontmatter:**
- `fallbackModel` — the agent `.md` parser ignores it (v2.1.195). Set it in settings.json (or CLI `--fallback-model`): an array of models tried in order when the primary is overloaded or unavailable

## Built-in Subagent Types

Available via Task tool's `subagent_type`:

- **Explore** — Haiku, read-only. Fast codebase search and analysis
- **Plan** — Inherits model, read-only. Codebase research for planning
- **general-purpose** — Inherits model, all tools. Complex multi-step tasks
- **Bash** — Inherits model. Terminal commands in separate context

Custom agents defined in `.claude/agents/` or `~/.claude/agents/` are also available as subagent types.

`subagent_type` matching is case- and separator-insensitive — `"Code Reviewer"` resolves to `code-reviewer` (v2.1.140+). Sub-agents can spawn their own sub-agents, up to 5 levels deep (v2.1.172+). Permission rules can constrain spawns: `Agent(type)` deny rules, `Agent(x,y)` allowed-types, and `Tool(param:value)` matching such as `Agent(model:opus)` (v2.1.178+/v2.1.186+).

## Interacting With Running Agents

- Agent tool no longer accepts a `resume` parameter. Use `SendMessage({to: agentId})` to continue a previously spawned agent
- `SendMessage` auto-resumes stopped agents in the background — no error on stopped agents
- `SendMessage` relayed from another Claude session carries no user authority — the receiver refuses relayed permission requests (v2.1.166+)
- `TaskOutput` is deprecated. Use `Read` on the background task's output file path instead

**Agent teams** (experimental, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`): `TeamCreate` and `TeamDelete` are removed (v2.1.178+). Every session has one implicit team — spawn teammates directly with the Agent tool's `name` parameter, no setup step. The `team_name` parameter is accepted but ignored.

## Agent-Only Skills

A skill hidden from the main conversation yet preloaded into one agent is NOT achievable via frontmatter.

**Preload resolves against the visible listing.** An agent's `skills:` preload matches names only against the model-invocable `available_skills` set — the same listing the main agent sees. A skill is preloadable if and only if it appears there. Hide it and the loader warns `Skill '<name>' specified in frontmatter was not found` and skips it. Both `disable-model-invocation: true` and `paths:` (which holds a skill out of the default listing until a matching file is in context) hide a skill, so both also make it non-preloadable.

**Empty `description:` does not hide.** The listing falls back to the skill BODY as its description, so an empty-description skill leaks its whole body into `available_skills` every session — the opposite of hidden.

Consequence: no frontmatter combination makes a skill both hidden from the main agent and injectable into a subagent.

```yaml
# agents/code-reviewer.md — preloads a normal (visible) skill
---
name: code-reviewer
skills: review-checklist
---
```

What `skills:` preload buys a subagent: it carries no `references/` of its own and runs no scripts. A preloaded skill gives it both — the skill's `references/` become available, and its `!`-commands run at load to inline their output, no LLM tool call or agent tokens (see building-skills.md). Preload fires on subagent dispatch (Task/Agent tool); running an agent as the main session via `--agent` does not preload its `skills:` bodies.

## Persistent Memory

Enable cross-session learning with `memory` field:

- `user` — `~/.claude/agent-memory/<name>/` — across all projects (recommended default)
- `project` — `.claude/agent-memory/<name>/` — project-specific, version-controllable
- `local` — `.claude/agent-memory-local/<name>/` — project-specific, not in VCS

Include memory instructions in the system prompt so the agent proactively maintains its knowledge base.
