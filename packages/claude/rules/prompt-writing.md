---
paths:
  - "**/Claude.md"
  - "**/CLAUDE.md"
  - "**/Domain.md"
  - "**/SKILL.md"
  - "**/skills/*/references/*.md"
  - "**/agents/*.md"
  - "**/commands/*.md"
  - "**/rules/*.md"
  - "**/Architecture.md"
---

### Write Prompts through /cc
Use /cc and follow its Process. Read Domain.md and the Prompt Architecture before writing,
and place every block per the Architecture's allowance. Trace every term to Domain.md or the
code — a term that traces to neither is coined; consult the Architect instead of writing it.

### Escalate a new Prompt file, file type, or name to the Architect
A new Prompt file, a new file type, or a new name is an Architectural Decision. The Architect
makes it.

### Write Prompts in the communication style
Prompt text follows every Rule in communication.md, Simplified Technical English first.

### Write Prompt text as declarative sentences
A Rule states what is, in plain words. No metaphor, no analogy, no figurative
language, no rhetorical question, no test question the reader must interpret.
Example: "Row-level security protects data a tenant owns."
Never: "a door cannot stand behind the fence it opens".

### Keep Evidence out of Prompt text
A Rule carries at most one grounding clause and a pointer to its record.
Measurements, probes, and failed alternatives live in
docs/architecture/decisions/ or the experiment record, never inline.
