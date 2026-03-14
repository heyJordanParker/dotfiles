
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
- `model` — `opus`, `sonnet`, `haiku`, or `inherit` (default)
- `skills` — skills injected into agent context at startup
- `permissionMode` — `default`, `acceptEdits`, `dontAsk`, `bypassPermissions`, `plan`

**Advanced:**
- `maxTurns` — max agentic turns
- `mcpServers` — MCP servers for this agent
- `hooks` — lifecycle hooks scoped to this agent
- `memory` — persistent memory: `user`, `project`, or `local`
- `background` — `true` to always run in background. Killing a background agent preserves partial results in conversation context
- `isolation` — `worktree` for git worktree isolation. Use `worktree.sparsePaths` setting in large monorepos to check out only specific directories

## Built-in Subagent Types

Available via Task tool's `subagent_type`:

- **Explore** — Haiku, read-only. Fast codebase search and analysis
- **Plan** — Inherits model, read-only. Codebase research for planning
- **general-purpose** — Inherits model, all tools. Complex multi-step tasks
- **Bash** — Inherits model. Terminal commands in separate context

Custom agents defined in `.claude/agents/` or `~/.claude/agents/` are also available as subagent types.

## Agent-Only Skills

Skills invisible to the main conversation — only loaded by specific agents.

1. Create skill with empty `description:` frontmatter — won't appear in available_skills
2. Agent declares it via `skills:` frontmatter — injected at startup

```yaml
# skills/review-checklist/Skill.md — agent-only (empty description)
---
name: review-checklist
description:
---

# agents/code-reviewer.md — loads the skill
---
name: code-reviewer
skills: review-checklist
---
```

**Why:** Main agent can't randomly load skills designed for specific agent contexts.

## Persistent Memory

Enable cross-session learning with `memory` field:

- `user` — `~/.claude/agent-memory/<name>/` — across all projects (recommended default)
- `project` — `.claude/agent-memory/<name>/` — project-specific, version-controllable
- `local` — `.claude/agent-memory-local/<name>/` — project-specific, not in VCS

Include memory instructions in the system prompt so the agent proactively maintains its knowledge base.
