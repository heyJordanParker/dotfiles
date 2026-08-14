---
name: curate-research
description: After a research thread completes, lift its durable knowledge into the wiki as readable prose. Read only that thread's research folder, select the lasting facts about a company, a market, a person, or a source, write them into the owning wiki topics, and report what was filed and what was left transient. TRIGGER when a finished research thread's findings should reach the wiki. DO NOT TRIGGER to file a session's loose captures with their grades and dates (that is journal), to consolidate topics (that is dream), or to gather new research (the research skills).
---

# Curate Research

One Process: read a finished research thread's folder, keep only the knowledge that stays true after the project ships, and write it into the wiki as pages a person reads and edits.

Inputs: the thread's `research/` folder and the wiki root — nothing else. The wiki root comes from the invocation or the calling context. A dispatched curator cannot ask: if the root is missing, escalate it to the dispatcher as a blocking gap and return nothing filed. Never open a judged workspace file — `Buyers.md`, `Decisions.md`, the plan files, the proposal, the deliverable — those carry this project's judgments, and reading them poisons a wiki that must hold only what is true outside this project.

## 1. Read the thread's research folder

Read the thread's `research/<subject>/` topic files — the records the thread wrote, one file per subject, no index and no capture files. This folder is your whole input. Read nothing the Inputs section excludes.

## 2. Select the durable entries, on input

Classify every entry as durable or transient before you write anything. Only durable entries reach the wiki; transient entries stay in the workspace where their judgment lives.

Durable — a fact that stays true after this project ships:
- A fact about a company: its offer, pricing, positioning, funnel, claims, ads.
- A fact about a market: who lives with the problem, the problem in the buyers' own words, the workarounds they reach for.
- A fact about a person: who they are as shown on the page.
- A fact about a source: what it is, where the buyers talk, what it trusts.
- The verbatim words and stated opinions of the real people the research quotes.

Transient — tied to this project's decision, never wiki material:
- Our ratings and rankings (a source's or a problem's 1-to-100 rating).
- A problem's rank, a chief-assumed call, an angle's strength, "which of these we should use".
- Anything that only means something inside this project's plan.

Example (keep): a buyer's "one misreported $253 order is a lot of money for me now" — a stated opinion, durable.
Example (drop): "this problem scored 72 and is the strongest we found" — our judgment, transient.

### Carry the fact, not the score we gave it
When a durable fact arrives with our rating attached, write the fact and strip the score. A source's own numbers read from its page — review counts, member counts, pricing — are facts about the source and stay; the 1-to-100 rating we assigned it is our judgment and goes.

## 3. File each durable entry into its owning topic as prose

Use the wiki skill to find the root, search before crawling, and read the owning topic. Route each fact to its branch — `topics/markets/<market>/` for market findings, `topics/companies/<name>/` for a competitor, `topics/products/<product>/` for product-specific material, `topics/copywriting/` for craft. Prefer an existing topic; create one only when nothing owns the fact, named in plain words, and name every topic you create in the report.

Fold the fact into the topic's `Claude.md` as prose:

- Readable pages organized by what the topic is about, matching the voice already on the page.
- Light frontmatter at most. No metadata tables, no formal citation blocks.
- Link the source inline where it reads naturally — `[name](topics/<topic>/<file>)`, relative from the wiki root, never absolute.
- No dates forced into the sentences.

### Fold into what is there, never overwrite a contradiction
Add to the page's existing knowledge. When a new fact contradicts what the page says, keep both and note the contradiction — never silently overwrite.

### Never write an agent's opinion onto a page
The page carries facts and the studied people's stated words. It never carries one of our agents' opinions or judgments. If a sentence you are about to write is our read on the material rather than a fact or a quoted person, it is transient — leave it out.

## 4. Re-index

`python3 scripts/wiki.py index` at the wiki root so search and the orphan report see the new prose. In an orchestrated run where the dispatcher reserved the index run, skip this and say so.

## 5. Report

What durable knowledge was filed into which topics, which topics were created, which contradictions were noted, and what was left as transient and why — so the dispatcher sees the judgment, not just the writes.

Verification: you read only the thread's `research/` folder and no judged workspace file; every entry written is a fact or a studied person's stated words, never our grade, rank, or opinion; every page reads as prose with inline relative source links and no metadata tables or forced dates; contradictions were kept, not overwritten; `wiki.py index` ran last unless an index run was reserved for the dispatcher.
