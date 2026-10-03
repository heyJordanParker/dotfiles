### Prefer dedicated tools over Bash
Prefer dedicated tools over Bash when one fits: Read, Edit, and Write. Reserve Bash for shell-only work.

### Run independent tool calls in parallel
Make independent tool calls in parallel — one message, multiple tool uses. Only sequence when one call's output feeds the next. Dispatching N parallel Subagents means N calls in one message, never serialized.

IF using remote, production, or staging access:
### Keep diagnostics read-only
Read logs, status, system information, config files, and read-only database queries. Do not restart services, kill processes, mutate data, edit files, attach debuggers, or print secrets unless the Architect explicitly approves mutation.

IF answering needs more than three searches of the codebase:
### Dispatch the explorer Agent
Search with `trace` yourself when fewer searches answer it, and never repeat research an Agent is already running.

IF the question is what was said, decided, or preferred before this session:
### Ask Memory with `honcho peer chat`
`honcho peer chat "<question>" -p <peer> --scope <project>` answers from what Memory holds about a peer in this project, and `honcho peer search "<query>" -p <peer>` returns the messages behind it. The injected block is a summary, not the record. Peers are `jordan` and one per Agent and model, such as `cto-claude-opus-5-5`. Each project is a scope named for its repository, such as `dotfiles`.

IF you learn something that holds in every project:
### Keep it with `honcho conclusion create`
`honcho conclusion create "<text>" --observer <peer> --session general` keeps one line about that peer, yours or `jordan`'s, and every project reads it. What holds in one project needs no command: Memory learns it from the conversation.
Never: a project's names, modules, or files, session context, or a one-time fix.

### Read, search, and list repository files with /trace
Use `trace read`, `trace grep`, `trace find`, `trace list`, `trace status`, `trace history`, and `trace blame` for repository files, and read their output whole. A raw `cat`, `grep`, `ls`, or `git log` on a repository path, or a pipe on trace output, is refused.
