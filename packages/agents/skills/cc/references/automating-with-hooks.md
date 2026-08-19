# Hooks

The Process for making the Harness react to an event with a Hook.

## 1. Decide whether the behavior belongs in a Hook

- A Hook is automation the Harness runs at a fixed event in the Agent's run.
- A deterministic command Hook is the Rule enforced in code, so duplicate prose is cut.
- A model-backed Hook batches the Rules for one event into one call and fails open, so the prose fallback stays in an always-loaded Prompt.
- This repository's local Hook source is `packages/agents/hooks/<module>.py`.
- `scripts/hooks.py` reads each Python Hook's `BINDING` and writes the Claude Code and Codex hook wiring.
- Plugin consumers get shell Hooks from `packages/claude/hooks/` through plugin packaging.

### Use a Hook when the behavior must hold at event time
Always-loaded prose is forgotten after a few turns. A Hook runs when the event happens.

### Keep deterministic Hook prose out of Prompts
When the command Hook blocks or rewrites the event deterministically, the Python file is the Rule. Keep only the Fact that the Hook exists if Agents need to know it.

### Keep model-backed Hook prose as fallback
Prompt and Agent Hooks can fail open, so the same Rule still needs a prose home where the Agent will see it.

## 2. Pick the Hook type and event

- `type: "command"` runs a shell command or script and is deterministic.
- `type: "prompt"` runs one model-backed turn with no tools.
- `type: "agent"` spawns a full Subagent with tool access.
- `type: "http"` delegates to an external HTTP service.
- `type: "mcp_tool"` invokes a Model Context Protocol tool directly (v2.1.118+).
- `PreToolUse` validates, blocks, or modifies tool calls; its matcher is tool names.
- `PostToolUse` reacts to tool results; its matcher is tool names.
- `PostToolUseFailure` fires when a tool call fails and carries `duration_ms` (v2.1.119+).
- `Stop` checks completeness before the Agent stops.
- `SubagentStart` fires when a Subagent is dispatched and accepts command-type Hooks only.
- `SubagentStop` validates Subagent completion.
- `TaskCompleted` reacts to completed Subagent Tasks.
- `UserPromptSubmit` adds Context or validates the user's Prompt.
- `MessageDisplay` transforms or hides displayed text (v2.1.152+).
- `SessionStart` loads Context or sets environment variables; its matcher is `startup`, `resume`, `clear`, or `compact`.
- `SessionEnd` runs cleanup.
- `ConfigChange` fires when settings hot-reload.
- `Setup` runs repository setup or maintenance for `--init`, `--init-only`, or `--maintenance`.
- `StopFailure` fires when a turn ends because of an API error and is not the same as `Stop`.
- `TaskCreated` fires when a Task is created through TaskCreate.
- `CwdChanged` fires when the working directory changes.
- `FileChanged` fires when a watched file changes.
- `PreCompact` fires before compaction; matcher values are `manual` and `auto`, and exit 2 or `{"decision":"block"}` can block compaction (v2.1.105+).
- `PostCompact` fires after compaction.
- `Notification` reacts to User notifications.
- `PermissionRequest` reacts to approval prompts for file access, tools, and user interaction.
- `PermissionDenied` fires after an auto-mode classifier denial, and `{retry: true}` tells the model it may retry (v2.1.89+).
- `Elicitation` intercepts Model Context Protocol elicitation requests before showing them to the User.
- `ElicitationResult` modifies elicitation responses before sending them back to the Model Context Protocol server.
- `WorktreeCreate` replaces default git worktree behavior and must return the worktree path.
- `WorktreeRemove` cleans up after `WorktreeCreate`.
- `.worktreeinclude` is not processed when a custom `WorktreeCreate` Hook is configured, so the Hook must copy config files itself.

IF the decision can be made from the event JSON alone:
### Use a command Hook
Command Hooks are fast, deterministic, and do not spend model tokens.

IF judging natural language:
### Use a prompt Hook
A prompt Hook runs one model-backed turn with no tools.
Template:
  ```json
  {
    "type": "prompt",
    "prompt": "Evaluate whether this tool use is safe. Check for system paths, credentials, and path traversal. Input: $ARGUMENTS. Return ONLY this JSON: {\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow|deny\",\"permissionDecisionReason\":\"...\"}}",
    "timeout": 30
  }
  ```
Never: `{"decision": "approve|block"}` from a PreToolUse prompt Hook — since v2.1.212/v2.1.214 it fails schema validation, renders a Hook error, and halts the turn instead of feeding the reason back. `deny` blocks the tool and returns `permissionDecisionReason` to the Agent so it retries; `ask` forces the User to intervene.

IF the Hook needs to inspect files or the codebase before deciding:
### Use an Agent Hook
An Agent Hook spawns a full Subagent with tools. Reserve it for high-value work where the automation justifies the overhead.
Template:
  ```json
  {
    "type": "agent",
    "prompt": "Check whether the file being written has corresponding tests. Input: $ARGUMENTS. If no tests exist, block with reason.",
    "model": "fast-model-id",
    "timeout": 60,
    "statusMessage": "Verifying test coverage..."
  }
  ```

### Use command-type Hooks for `SessionStart`, `Setup`, and `SubagentStart`
`SessionStart`, `Setup`, and `SubagentStart` reject prompt-type and agent-type Hooks with an error that says to use a command-type Hook instead (v2.1.142+).

## 3. Write the wiring

- Claude Code loads Hook settings by precedence: `~/.claude/settings.json`, `.claude/settings.json`, `.claude/settings.local.json`, plugin `hooks/hooks.json`, then component frontmatter.
- `once: true` runs a Hook only once per session.
- `args: [...]` uses exec form for command Hooks and spawns the command directly without a shell, so path placeholders need no quoting (v2.1.139+).
- `continueOnBlock: true` applies to `PostToolUse` only and feeds the rejection reason back to Claude while continuing the turn instead of erroring (v2.1.139+).
- `if: "ToolName(pattern)"` spawns a handler only when the tool call matches the pattern; it uses permission Rule syntax and requires v2.1.85+.
- `Bash(git *)` matches Bash commands starting with `git`.
- `Edit(**/Claude.md)` matches `Claude.md` at any depth.
- `//` means filesystem root, `~/` means home, `/` means project root, and a bare path is current-working-directory relative.
- `*` matches within one directory, and `**` matches recursively.

Template:
  ```json
  {
    "hooks": {
      "<EventName>": [{
        "matcher": "<ToolName>",
        "hooks": [{ "type": "command", "command": "<script>" }]
      }]
    }
  }
  ```

Example:
  ```json
  {
    "hooks": {
      "PreToolUse": [{
        "matcher": "Write|Edit",
        "hooks": [{ "type": "command", "command": "./validate.sh" }]
      }]
    }
  }
  ```

### Use separate handler entries for Write and Edit inside `if`
No pipe alternation exists inside `if`, so each tool gets its own handler entry.
Never: `{ "if": "Write|Edit(**/Claude.md)" }`.
Template:
  ```json
  {
    "matcher": "Write|Edit",
    "hooks": [
      { "type": "command", "if": "Write(**/Claude.md)", "command": "./validate.sh" },
      { "type": "command", "if": "Edit(**/Claude.md)", "command": "./validate.sh" }
    ]
  }
  ```

### Define scoped Hooks in skill and agent frontmatter only when the scope is real
Frontmatter Hooks run only when that Skill or agent is loaded. Use settings JSON for repository-wide behavior.
Template:
  ```yaml
  ---
  hooks:
    PreToolUse:
      - matcher: "Write"
        hooks: [{ type: "command", command: "./validate.sh" }]
  ---
  ```

## 4. Match tool calls exactly

- `"Write"` matches one exact tool.
- `"Write|Edit"` matches multiple tools as canonical alternation.
- `"Bash,PowerShell"` also fires as a comma-separated matcher as of v2.1.191.
- `"*"` matches all tools.
- `"mcp__.*"` matches by regular expression.

### Use the literal `"*"` matcher for all tools
Observed in 2.1.131: omitting the matcher field did not fire reliably across tools, even though the docs treated an omitted matcher as equivalent to `"*"`.

### Match hyphenated identifiers exactly
Hyphenated identifiers exact-match in v2.1.195+. A matcher such as `code-reviewer` or `mcp__brave-search` no longer substring-matches. To match all tools from a hyphenated Model Context Protocol server, use `mcp__brave-search__.*`.

### Verify tool names with `/hooks`
Matchers are case-sensitive. Use `/hooks` to confirm the tool name Claude Code sees.

## 5. Handle exit codes and event input

- Exit code 0 succeeds and processes stdout.
- Exit code 2 blocks and shows stderr to Claude or the User.
- Any other exit code is a non-blocking error except where the event says otherwise.
- `PreToolUse` exit 2 blocks the tool call and shows stderr to the Agent.
- `PreToolUse` can satisfy `AskUserQuestion` by returning `updatedInput` with `permissionDecision: "allow"`.
- `PreToolUse` can return `permissionDecision: "defer"` to pause a headless session at the tool call so `-p --resume` re-evaluates the Hook (v2.1.89+).
- `PreToolUse` Hooks returning `"allow"` do not bypass `deny` permission Rules, including enterprise managed settings.
- A `deny` Rule overrides a Hook's `permissionDecision: "ask"` rather than becoming a prompt (v2.1.101+).
- `UserPromptSubmit` exit 2 blocks Prompt submission and shows stderr to the User.
- Any non-zero `UserPromptSubmit` exit blocks submission.
- `Stop` exit 2 blocks stopping; stderr becomes instructions to the Agent, which acts on them and tries to stop again.
- `Stop` can fire multiple times per turn; the turn ends with a warning after eight consecutive blocks unless `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` overrides it (v2.1.143+).
- Every Hook event receives a JSON object on stdin.
- All Hook events carry `effort.level`, which command Hooks and Bash tool commands also receive as `$CLAUDE_EFFORT` (v2.1.133+).
- `session_id` is the session UUID.
- In Claude Code 2.1.131, `session_id` is the parent session's UUID even when the tool runs inside a Subagent; this was verified by reading Hook payloads from `/tmp/payload-trace.log` during a controlled Subagent dispatch.
- Subagent identity is carried by `agent_id` and `agent_type`, not by `session_id`.
- `agent_id` appears only when the event fires inside a Subagent execution.
- `agent_type` appears with `agent_id` and gives the dispatched Subagent type.
- `tool_name` is the tool being called.
- `tool_input` carries tool-specific parameters.
- `cwd` is the working directory.
- `duration_ms` appears on `PostToolUse` and `PostToolUseFailure` and excludes permission prompts and `PreToolUse` Hooks (v2.1.119+).
- `UserPromptSubmit` carries `session_id`, `prompt`, and `transcript_path`.
- `Stop` carries `session_id`, `transcript_path`, `cwd`, `last_assistant_message`, `stop_hook_active`, `permission_mode`, `hook_event_name`, `background_tasks`, and `session_crons`.
- `PermissionRequest` carries `session_id` and `tool_name`.

### Route per-tool-call events per Subagent
Overwrite `SESSION_ID` with `agent-<agent_id>` when the payload carries `agent_id`.
Template:
  ```bash
  SESSION_ID=$(echo "$INPUT" | jq -r '.session_id')
  AGENT_ID=$(echo "$INPUT" | jq -r '.agent_id // empty')
  [ -n "$AGENT_ID" ] && SESSION_ID="agent-$AGENT_ID"
  ```

### Guard system-generated `UserPromptSubmit` messages structurally
Background agent completions, Skill content injections, and interrupt notifications enter conversations as `type: "user"` and can trigger `UserPromptSubmit`. Detect shape instead of enumerating every tag.
Template:
  ```bash
  #!/bin/bash
  PROMPT=$(echo "$EVENT" | jq -r '.prompt // ""')

  if [[ "$PROMPT" == "<"* ]]; then
      TAG_REST="${PROMPT#<}"
      TAG_NAME="${TAG_REST%%[> ]*}"
      [[ -n "$TAG_NAME" && "$PROMPT" == *"</${TAG_NAME}>"* ]] && exit 0
  fi

  if [[ "$PROMPT" == "["* ]]; then
      FIRST_LINE="${PROMPT%%$'\n'*}"
      [[ "$FIRST_LINE" == *"]" && "$PROMPT" == "$FIRST_LINE" ]] && exit 0
  fi

  [[ "$PROMPT" == "This session is being continued"* ]] && exit 0
  [[ "$PROMPT" == "Base directory for this skill:"* ]] && exit 0
  ```
Example: this skips `<task-notification>`, `<teammate-message>`, `<command-name>`, `<local-command-*>`, `<system-reminder>`, `[Request interrupted by user]`, `[Image: source: ...]`, and future tagged system messages.

### Read project transcripts from `~/.claude/projects/`
`transcript_path` points to project JSONL files under `~/.claude/projects/<project-path>/<session-uuid>.jsonl`, not to `~/.claude/transcripts/`.

### Filter real User messages by excluding tool results
JSONL `"type":"user"` entries include real User messages and tool results. Exclude entries containing `tool_use_id` to keep only real User messages.

### Do not assume every transcript user entry fires `UserPromptSubmit`
The transcript stores message shape; the Hook system independently decides when to fire.

## 6. Return Hook output deliberately

- `UserPromptSubmit` injects Context by outputting JSON to stdout.
- `additionalContext` appears in the Agent's Context before it processes the User's Prompt.
- `PreToolUse` Hooks can also return `additionalContext`.
- `Stop` and `SubagentStop` can return `additionalContext` to give Claude feedback and keep the turn going without being labeled a Hook error (v2.1.163+).
- `PostToolUse` can return `updatedToolOutput` to replace tool output for any tool, not only Model Context Protocol tools (v2.1.121+).
- `SessionStart` can return `sessionTitle` and can set top-level `reloadSkills: true` to re-scan Skill directories so Skills installed by the Hook load in the same session (v2.1.152+).
- `UserPromptSubmit` can return `sessionTitle` (v2.1.94+).
- Top-level `terminalSequence` emits desktop notifications, window titles, and bells without a controlling terminal (v2.1.141+).
- Hook output over 50K characters is saved to disk and replaced with a file path plus preview instead of being injected directly into Context (v2.1.89+).
- `permissions.additionalDirectories` in settings JSON is equivalent to `--add-dir` and accepts relative paths.

### Inject Context from `UserPromptSubmit` with JSON stdout
Return `hookSpecificOutput.additionalContext` for `UserPromptSubmit`.
Template:
  ```json
  {"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"text injected as system-reminder"}}
  ```

### Do not make a Hook invoke a `disable-model-invocation` Skill
A Hook cannot make the Harness invoke a `disable-model-invocation` Skill through the Skill tool. Injecting `Load /<skill> now via the Skill tool` as `additionalContext` routes the model to the same gate and fails with `Skill <name> cannot be used with Skill tool due to disable-model-invocation`.

### Name the Skill, and expect no text back on a second use
The Skill tool answers a second use with `Skill /<name> is already loaded above; instructions unchanged`. That is the Harness saying the Process is still in the conversation, not a failure: an order to use it again buys the Agent going back to the steps, never the text arriving twice. Verified at 2.1.195 on `/5-whys` and `/cc`.

### Inline content only when the Hook accepts the loss of Skill machinery
A Hook can inline that Skill's body as plain `additionalContext`, but that carries no `allowed-tools`, `references/`, `!` autorun, or Skill registration. See `building-skills.md` for the `disable-model-invocation` flag.

Example: `{ "permissions": { "additionalDirectories": ["../docs/"] } }`

## 7. Debug and verify the Hook

- `disableAllHooks` and `allowManagedHooksOnly` gate whether Hooks run at all.
- An unrecognized Hook event name in settings JSON no longer invalidates the whole file (v2.1.101+).
- `$CLAUDE_PROJECT_DIR` is the project root.
- `$CLAUDE_ENV_FILE` is available during `SessionStart` to persist environment variables.
- `$CLAUDE_EFFORT` is the active effort level for command Hooks and Bash tool commands (v2.1.133+).
- `$CLAUDE_CODE_SESSION_ID` matches the `session_id` passed to Hooks and is also set in Bash tool subprocesses and stdio Model Context Protocol servers (v2.1.132+).
- `$CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1` strips Anthropic and cloud credentials from subprocess environments.
- Supported prompt, Agent, and HTTP Hook events are `PreToolUse`, `PostToolUse`, `Stop`, `StopFailure`, `SubagentStop`, `UserPromptSubmit`, `PermissionRequest`, `TaskCompleted`, `TaskCreated`, `CwdChanged`, `FileChanged`, `Elicitation`, and `ElicitationResult`.
- An Agent Hook requires `type: "agent"` and `prompt: string`; `model`, `timeout`, `statusMessage`, and `once` are optional.
- Agent Hooks default to 60 seconds, while prompt Hooks default to 30 seconds.
- HTTP Hooks post Hook input JSON to the URL and expect the same decision JSON back.
- Observed in 2.1.220: `SubagentStart` and `SubagentStop` both fire, `SubagentStop` carries `agent_id`, `agent_type`, `agent_transcript_path`, and `last_assistant_message`, and a `SubagentStart` is not guaranteed a matching `SubagentStop`, since an observed failed dispatch emitted only `SubagentStart`.
- Subagent inner tool calls do fire `PreToolUse` and `PostToolUse` with the parent's `session_id` and the Subagent identity in `agent_id` and `agent_type`.
- Official Hook docs live at <https://code.claude.com/docs/en/hooks>.

IF your Hook never fires:
### Check `disableAllHooks` and `allowManagedHooksOnly` first
A disabled Hook system makes matcher debugging irrelevant.

### Restart after Hook changes
Hooks load at session start. Changes require restart. Use `/hooks` to verify registration.

### Verify registration with `/hooks`
Run Claude Code with debug logging when `/hooks` does not explain the behavior.
Example: `claude --debug` and filter for Hook output.

### Emit Hook lifecycle events while debugging
`claude -p --output-format=stream-json --include-hook-events` emits Hook lifecycle events in the output stream.

### Make Hooks independent
Hooks run in parallel with no guaranteed order. Do not rely on shared mutable state between Hooks.
