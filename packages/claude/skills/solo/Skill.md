---
name: solo
description: Framework for working solo — no subagents, deep full-file reads, thorough research before acting. Use when context window can absorb the full picture. For agent-based work, use /subagents or /team.
---

# Solo

No subagents, no delegation. Read the full architecture before touching it — fix or propose solutions based on root causes at the right layer, not symptoms at the wrong one. For parallel independent tasks, use /subagents. For persistent multi-task coordination, use /team.

## Research

- The trace skill is how you research and how you locate files and symbols — not raw Read or grep. Once located, read the file in full through it
- Read whole files. Never use offset/limit on files under 500 lines
- Follow every reference — imports, callers, siblings, tests, configs
- Read the surrounding architecture before proposing changes. The file with the symptom is rarely the file with the problem

## Hard Rules

- No Agent tool — enforced by the `enforce-solo-mode` hook
- No skimming — if a file matters, read it fully
- No hedging — never say "likely" or "probably" about code you can read
- Assume existing code is intentional — if the reason for something is unclear, research more before changing or proposing changes

## Process

1. **Locate** — find the relevant files
2. **Read** — every relevant file fully, plus surrounding architecture
3. **Assess** — is this a real issue? Trace the code path to confirm. If not, report findings and stop
4. **Understand** — which layer owns this responsibility? Fix it there, not where the symptom surfaces
5. **Propose or act** — at the correct abstraction layer
6. **Verify** — trace changes through the code paths you read. Run tests
