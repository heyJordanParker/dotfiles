---
name: understand
description: Deep research of the systems behind a change — name the systems, show the architecture, read the git history for intent, trace the code, attack the understanding, and report in a layered file another Agent can act on coherently. TRIGGER when a change lands in a system the Agent has not mapped, when the Architect says "understand the system first", "research the architecture", "deep research", or "how did this come to be", and when a dispatch brief asks for system research. DO NOT TRIGGER for external library or vendor research (that is /research), for one symbol or file lookup (that is /trace), or for orchestrating research Subagents into an unknown situation (that is /progressive-research).
---

# Understand

- The next Agent acts on how the system works and why it was built that way.
- The Process is read-only; its one written file is the report in step 6.
- Your own reads produce the understanding; a Subagent's summary does not.
- Every read, search, and listing in this Process runs on trace commands.

## 1. Name the systems

### List each system before reading any implementation
From the tree itself — `trace survey`, `trace tree`, the entry points — list each system the task touches: its owner module, its one-sentence job, the modules it calls, and the modules that call it. A system you cannot describe this way is not yet safe to read deeply.
Never: opening the file the task names and reading outward from it.

### Verify the task's premise
The task arrives with a premise about the system. The premise is a claim like any doc: confirm it against the code. When the code disagrees, the correction is the report's first finding, and the change gets planned against what the code shows.

### Rank every doc below the code
When code and documentation disagree, the code is authoritative. Architecture.md, where it exists, is a starting hypothesis for the system list. Confirm every doc claim in code before it enters the report; a contradicted claim is a stale-doc finding.

## 2. Show the architecture

For each named system, show its shape: the public surface (`trace structure`, `trace symbols`), what depends on it (`trace downstream`), what it depends on (`trace upstream`), and where its boundary with each neighbor sits.

### Show each system as an annotated file tree
One tree per system, per /show-me: each file with a role note under nine words, contracts and boundaries named in the notes.

### Name the contracts between systems
For each pair of systems that touch, state the contract in one sentence: who calls whom, with what, and who owns the data.

## 3. Read the history

Research each system's git history.

### 3.1 Find the design-changing commits
Scan the subjects for the commits that changed the system's design.

Template:
  ```bash
  trace history <module-path>
  trace blame <file> [<symbol>]
  ```

### 3.2 Read those commits' bodies
Read the full body of each design-changing commit alone. The subject says what changed; the body says why, what was rejected, and which invariant the change protects. The body is the primary source of intent. No trace command returns a body, so this is the one direct git command in the Process.

Template:
  ```bash
  git show -s --format=full <commit>
  ```

### 3.3 Build the decision timeline
Order the design-changing commits into a timeline: date, commit id, the design decision it made, and the problem that forced it. Record every rejected design by name — a design the history rejected must not return through the next change.

### 3.4 State each system's maturity
From the timeline: how old the design is, how many times it was reshaped, and whether it has held since the last reshaping. A design that survived many reshapings constrains future changes more than a new design does; a design one commit old is open to a contract-change proposal. The implementing agent calibrates how freely it may propose changes from this.

## 4. Trace the code

Only now read the implementation behind the task with `trace read`: whole files, the full call chain, every branch on the path. Ground every claim about behavior in a line you read this pass.

### Fill no blank with a plausible guess
Close missing understanding with another read, or list it as an open question. Never infer it.

## 5. Attack

### 5.1 Verify the invariants
State the invariants the system protects — the conditions that must stay true — and verify each against the code. Make three predictions about behavior the task will touch and confirm each in the source. A prediction that fails means the understanding is wrong: go back to the step that produced it.

### 5.2 Challenge the design against industry precedent
With the history in hand, ask /5-whys of the architecture: why does each major part exist at all? Then compare the design with the proven industry solution for the same problem and name every divergence. A divergence the history explains is a recorded Decision; a divergence with no recorded reason is a finding for the report.

## 6. Report

The report is written for a reader who has not read the code and will not read it. Write it to `docs/agents/<NNN>-<task-slug>/research.md`, then deliver it whole in the reply. Write it in the project's words: Domain.md, where it exists, is the vocabulary.

Template:
  ```markdown
  ---
  research: true
  ---

  ## The premise, checked
  What the task assumed, what the code shows, and what the change is actually against. One paragraph; when the premise held, one sentence.

  ## Why this system exists
  What it does for the User and the business, in 2-4 sentences. No file names.

  ## The architecture
  The annotated file trees, the contracts between systems, and the boundaries. What owns what.

  ## How it came to be
  The decision timeline: date, commit, decision, forcing problem. The rejected designs. Each system's maturity: age, reshapings survived, and what that licenses the implementing agent to touch.

  ## The mechanics behind this task
  The specific call chain, state machine, or data flow the task will change. Exact names and paths. End with the code paths and triggers the smallest implied change activates, verified in the selection and trigger code.

  ## Findings
  What research turned up that the task did not ask about: stale docs, divergences from industry precedent with no recorded reason, latent defects, asymmetries between paths that should match. Each traced to the code that shows it.

  ## Invariants and open questions
  The conditions that must stay true, each traced to code or a commit and verified this pass. Then every question research could not close, as a question — never a guess.
  ```
