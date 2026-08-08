### Prefer dedicated tools over Bash
Prefer dedicated tools over Bash when one fits: Read, Edit, and Write. Reserve Bash for shell-only work.

### Run independent tool calls in parallel
Make independent tool calls in parallel — one message, multiple tool uses. Only sequence when one call's output feeds the next. Dispatching N parallel Subagents means N calls in one message, never serialized.

### Track multi-step work with TaskCreate
Use TaskCreate to track multi-step work. Mark each Task completed the moment it is done, never in a batch.

IF using remote, production, or staging access:
### Keep diagnostics read-only
Read logs, status, system information, config files, and read-only database queries. Do not restart services, kill processes, mutate data, edit files, attach debuggers, or print secrets unless the Architect explicitly approves mutation.

IF broad codebase exploration needs more than three queries:
### Dispatch the explorer Agent
Dispatch the explorer Agent instead of continuing direct exploration.

IF external research is needed:
### Dispatch the researcher Agent
Dispatch the researcher Agent for library docs or APIs.

IF direct code research is enough:
### Use the trace Skill directly
Use the trace Skill directly. Subagents protect the main Context but are not free — do not spawn one where a direct call answers faster, and do not duplicate research a Subagent is already running.

IF running Subagents:
### Run Subagents in the background
Run Subagents in the background so you can keep working while they run. Poll at most every 30 minutes of wall-clock time; short-interval polling burns the Context synthesis needs. If progress matters, have the Subagent emit milestone events.

IF the question is what was said, decided, or preferred before this session:
### Ask Memory with `honcho`
`honcho ask <peer> <question>` reasons over everything Memory holds about a peer, `honcho search <query>` returns the messages behind it, and `honcho context <peer> [query]` returns the stored conclusions. The injected block is a summary, not the record. Peers are `jordan` and one per Agent by its name.

IF the Architect tells you something about yourself that this session's Memory did not carry:
### Keep it with `honcho remember`
`honcho remember <text>` keeps one line in your own collection. Never name the Agent; the running Agent is resolved for you.

IF a Decision depends on repo or code state:
### Check current state with trace
Check current state now with `trace`: `trace status`, `trace history`, or `trace blame`. Do not use raw `git status`, `git log`, or `git diff`, which give a bare list without callers, complexity, or dependents.
