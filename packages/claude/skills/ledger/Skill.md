---
name: ledger
description: Review and update Claude.md files — check template compliance, surface missing requirements/boundaries, and propose ledger entries.
context: fork
---

# Ledger

Review and update Claude.md files against the Why/What/How template.

## Template Reference

Read `/cc` reference `claude-md.md` for the full template before making any changes.

## Claude.md Changes

!`git diff HEAD --stat -- '**/Claude.md'`

## Modes

### Review (default)

`/ledger` or `/ledger path/to/dir`

1. Read the Claude.md at the target path (default: nearest to cwd)
2. Check template compliance:
   - **Why** section exists (min 1 sentence)
   - **What** section exists with Requirements and/or Boundaries
   - **Ledger** section exists (min 1 versioned entry)
3. Run `git log --oneline -20 -- <dir>` for the target directory
4. Identify:
   - Architectural decisions in recent commits without ledger entries
   - Requirements or Boundaries that may be stale based on recent changes
   - Missing template sections
   - Content that belongs at a different level of the hierarchy (too specific for parent, or duplicating parent in child)
5. Propose updates as diffs — one concept per diff

### Backfill

`/ledger backfill` or `/ledger backfill path/`

1. Find all Claude.md files under the target path: `git ls-files '**/Claude.md'`
2. For each non-compliant file:
   a. Read full contents
   b. Extract implicit requirements and boundaries from existing HOW sections
   c. Draft Why/What sections with Requirements and Boundaries
   d. Add initial Ledger entry
3. Present all proposals grouped by file as diffs
4. Apply approved changes

## Rules

- Only extract constraints that are **intentional architectural decisions**, not incidental implementation details
- Requirements use **must** and **always**
- Boundaries use **never**
- Ledger entries are keyed by the file's version number: `- v1.1: Description of decision`. Adding a ledger entry requires bumping the version; bumping the version requires a ledger entry. They enforce each other
- One entry per architectural decision in the commit — not per iteration step
- Short WHAT, then WHY when not obvious (for/because). Most entries under 10 words
- Never include HOW details — name the decision and motivation, not the implementation mechanics
- Amend existing entries when iterating before committing — the ledger matches what git shows (A→C, not A→B→C)
- Ledger records facts ("Replaced Nginx with Caddy"), not ongoing constraints — those go in Requirements or Boundaries
- A ledger entry is an architectural decision within this Claude.md's own documented scope — not a changelog of repo or code changes, which git records. Script, hook, or prompt changes the file only lists are repo history, not ledger entries, even when they happen in this directory
- One concept per change — don't restructure entire files in a single diff
- Preserve all existing content — restructure, don't delete
- When extracting from HOW sections: move the constraint to What, keep the implementation detail in How
- Read the full Claude.md file before proposing any edits — piecemeal edits without full context cause contradictions and repetition
- Never fabricate WHY in ledger entries — if the motivation is unknown, ask. "Chose X" requires knowing why X was chosen
- Ledger entries record decisions, not descriptions — don't repeat what the file already documents
- Ledger entries are one line — details belong in the file body or a deeper Claude.md, not the ledger
- Never include execution context in ledger entries — impact scores, importance ratings, and session metadata are meaningless outside the session
- Never place scope-specific entries in parent Claude.md files — a decision about backend belongs in the backend Claude.md, not root
