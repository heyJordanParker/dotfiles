### Prefer dedicated tools over Bash
Prefer dedicated tools over Bash when one fits: Read, Edit, and Write. Reserve Bash for shell-only work.

### Run independent tool calls in parallel
Make independent tool calls in parallel — one message, multiple tool uses. Only sequence when one call's output feeds the next. Dispatching N parallel Subagents means N calls in one message, never serialized.

IF using remote, production, or staging access:
### Keep diagnostics read-only
Read logs, status, system information, config files, and read-only database queries. Do not restart services, kill processes, mutate data, edit files, attach debuggers, or print secrets unless the Architect explicitly approves mutation.

IF broad codebase exploration needs more than three queries:
### Run `codex-run @explorer` in the background

IF direct code research is enough:
### Use the trace Skill directly
Use the trace Skill directly. Subagents protect the main Context but are not free — do not spawn one where a direct call answers faster, and do not duplicate research a Subagent is already running.

IF the question is what was said, decided, or preferred before this session:
### Ask Memory with `honcho`
`honcho ask <peer> <question>` reasons over everything Memory holds about a peer, `honcho search <query>` returns the messages behind it, and `honcho context <peer> [query]` returns the stored conclusions. The injected block is a summary, not the record. Peers are `jordan` and one per Agent by its name.

IF the Architect tells you something about yourself that this session's Memory did not carry:
### Keep it with `honcho remember`
`honcho remember <text>` keeps one line in your own collection. Never name the Agent; the running Agent is resolved for you.

### Record to Memory only what outlives the session
Memory records the Architect's corrections, recurring patterns, and conventions of your own craft that improve future runs.
Never: session context, one-time fixes, or content that belongs in Claude.md files.

### Read, search, and list repository files with /trace
Use `trace read`, `trace grep`, `trace find`, `trace list`, `trace status`, `trace history`, and `trace blame` for repository files, and read their output whole. A raw `cat`, `grep`, `ls`, or `git log` on a repository path, or a pipe on trace output, is refused.
