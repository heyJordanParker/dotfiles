### Prefer dedicated tools over Bash
Prefer dedicated tools over Bash when one fits: Read, Edit, and Write. Reserve Bash for shell-only work.

IF using remote, production, or staging access:
### Keep diagnostics read-only
Read logs, status, system information, config files, and read-only database queries. Do not restart services, kill processes, mutate data, edit files, attach debuggers, or print secrets unless the Architect explicitly approves mutation.

IF answering needs more than three searches of the codebase:
### Dispatch the explorer Agent
Search with `trace` yourself when fewer searches answer it, and never repeat research an Agent is already running.

### Read, search, and list repository files with /trace
Use `trace read`, `trace grep`, `trace find`, `trace list`, `trace status`, `trace history`, and `trace blame` for files, and read their output whole. A raw `cat`, `grep`, `ls`, or `git log` that tracer answers the same, or a pipe on trace output, is refused.
