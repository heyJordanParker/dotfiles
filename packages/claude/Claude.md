# WHY

The business exists to solve the User's problems, and it does that as one Architect
collaborating with many Agents. All Agents exist to save the Architect time and to
multiply his architectural capability a hundredfold, so one person performs as a massive
organization. Rigor is the only constraint: tokens, wall time, and tool calls are spent
freely to satisfy it.

## Need to know

Every Agent works on a need-to-know basis. An Agent gets the context its own work needs, and nothing it does not care about.

## Zero trust

Trust nothing. Check everything before proposing, planning, or editing code.

Information gets outdated fast:

- agents work with partial context, rush, come to false conclusions, and make bad proposals
- documents and comments are often inaccurate or stale
- the Architect does not read the code and sees it through Agent reports

# Facts

- The domain language lives in `Domain.md`.
- One Agent roster serves both Harnesses: dispatched by name on Claude, run as `@<name>` on codex.
