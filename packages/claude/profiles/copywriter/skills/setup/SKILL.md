---
name: setup
description: Initialize a product's workspace — interview the owner for the owner-fact files and the product folder, and scaffold the folder the system works in. TRIGGER when a product has no workspace yet, or a new product enters the system. DO NOT TRIGGER to scaffold one deliverable inside an existing workspace (start) or to plan a piece's copy (plan-copy).
---

# Set up a product

One Process: scaffold the workspace the owner points the system at, and capture what the owner states — the owner-fact files and the product folder. These are owner-stated facts ONLY, judgment-free — no research is needed first, so setup never stalls. Every file is the current state; git is the history. Every file starts minimal and legal; files grow by proposal, never front-loaded.

## 1. Scaffold the workspace

### Scaffold the owner-fact files, the product folder, the judged-file placeholders, and research/ at the workspace root
The workspace is the folder the owner points at — no wrapper subfolder. At its root create:

- The owner-fact files: `Business.md`, `Founder.md`, `Team.md` (only when a team exists), and `Voice.md`.
- `product/` — the owner's product facts: `product/Brief.md` (what the product is and the problems it solves, factual and short) and `product/Features.md` (the deep per-feature breakdowns, read only when a piece needs them). Both judgment-free.
- The judged-file placeholders, scaffolded empty: `Buyers.md`, `Competitors.md`, `Problems.md`, `Statistics.md`. Each holds ONE data type, filled later by the judged-files step from the research records — never by setup. Scaffold each with its one-line contract and no entries:
  - `Buyers.md` — the buyer hypotheses, each a 1-100 rating with reasoning and citations to research records.
  - `Competitors.md` — the competitors, each rated the same way.
  - `Problems.md` — the problems, each rated the same way.
  - `Statistics.md` — industry statistics ALONE, each rated the same way, so no strategist feels obliged to reach for a stat.
- `Decisions.md` — workspace-wide settled calls ONLY, empty placeholder. The chief is its sole writer.
- `OpenQuestions.md` — workspace-wide open questions ONLY, empty placeholder.
- `research/` — empty. Research threads — each `research/<subject>/`, named by its question at any granularity — are created inside it when research is commissioned, never at setup. The root `Brief.md` that commissions research (carrying the owner's problems in his words and their starting domains) is written when research is commissioned, not by setup.

## 2. Interview the owner for the owner-fact files

### Capture the owner's stated facts, one question at a time
The interview runs — it is not optional. It is skipped for one fact only when the owner already answered it and the answer is on file; otherwise ask. Write the owner's facts as he states them; minimal is legal.

Capture the PROBLEMS the owner voices, in his own words, as plain facts — a problem is just a problem, not owned, not sized, not validated — plus the starting DOMAINS those problems sit in (the spaces, categories, or situations they live in, "solo founders selling courses through their own funnels"). These two — the problems in his words and their starting domains — are what the root research `Brief.md` will carry when research is commissioned. Stating a problem is not claiming it is real or common: the research sweep enumerates what people actually voice, and the judged `Problems.md` rates the owner's problems as hypotheses among the rest. Also record the problems the product solves in `product/Brief.md` as owner-stated product facts. Capture the owner's pricing facts so the offer's exchange has a stated fact to draw on. Capture where readers arrive from — the owner's traffic sources and entry points — so the awareness read has a stated arrival context; where he cannot say, awareness stays open per piece, never defaulted. He cannot name the buyer: the market determines that, so no owner-fact file and no brief ever names a buyer.

### Hold every file to its contract
Each file holds exactly its contract, and every agent that later touches one is held to the same contract:

- `Business.md` — the business: numbers, team, useful facts the copy might need. Only facts the owner explicitly said or confirmed. No positioning, no offer shape, no buyer.
- `product/Brief.md` — what the product is and the problems it solves, factual and short. Live facts only — no future features, no selling language, no copy choices. Mechanisms, offers, and angles are assembled from the facts during the copywriting process; fixing them here kneecaps the copy agents.
- `product/Features.md` — the deep per-feature breakdowns behind the brief, judgment-free, read only when a piece needs them.
- `Founder.md` — the founder himself. No copy instructions.
- `Team.md` — the team, stated flat. No hedging ("primarily"): make the claim or interview the founder.
- `Voice.md` — the owner's personal preferences and verbatim examples of his copy, nothing else. Copywriting instructions live in the skills; process context lives in the process files.

A statement true of every product in the category is not a fact. An unanswered question is not decided: log it in `OpenQuestions.md`, never guess.

### Reference the speaker source in Voice.md
`Voice.md` MUST reference whoever the copy comes from — the source file that defines who is speaking: `Founder.md`, `Team.md`, or `Business.md` — plus how that speaker sounds. It never restates who they are; it points at their source file and captures the voice. A `Voice.md` with no referenced speaker source is incomplete. A `Voice.md` carrying no owner copy sample is also incomplete: flag it as an owner question in `OpenQuestions.md` before any draft is written — a voice cannot be matched against samples that do not exist.

### Record a stated absence as a fact
An absence the owner states is a fact: no customers yet, no results yet, no case studies yet. Record it in `product/Brief.md` worded as the owner stated it — "no customer results yet" — so proof planning reads a stated absence instead of a writer inventing a result to fill the gap. research-product's proof-of-operation step reads this to know the proof gap is real.

Verification: the workspace scaffolded at the folder the owner points at, no wrapper subfolder, carrying `Business.md`, `Founder.md`, `Team.md` (present when a team exists), `Voice.md`, `product/Brief.md`, `product/Features.md`, the four empty judged-file placeholders (`Buyers.md`, `Competitors.md`, `Problems.md`, `Statistics.md`), `Decisions.md`, `OpenQuestions.md`, and an empty `research/`; every file inside its contract; `Voice.md` references its speaker source and, if it carries no owner copy sample, is flagged in `OpenQuestions.md`; the problems in the owner's words and their starting domains captured, and every owner-stated fact captured, the buyer named nowhere; the judged files empty of any rating; nothing invented beyond what the owner stated.
