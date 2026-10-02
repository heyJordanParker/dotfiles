# Docs page through the hook: a doc too long for one message arrives in parts

Claude Code caps a hook message at 10,000 characters and a Bash result at 30,000, saving
anything longer to a file behind a 2,000-character preview. Tracer named every doc that did
not fit one message and never sent it, so the nearest doc, the one governing the file, was
the one Agents lost. Chosen: docs go nearest first, the first that does not fit is cut at a
whole line with `trace read`'s marker, and a doc counts as loaded only once every line
arrived, so each later `trace` call continues it. Rejected: sending whole and letting Claude
Code cut it (the Agent sees 2,000 characters), and raising `bashOutputMaxChars` to carry docs
in the trace result (lifts the cap for every command; a Subagent's shell has no agent id).
Cost accepted: a 54,000-character doc takes six `trace` calls to arrive whole.
