---
name: judge-research
description: Turn a completed research phase into the four judged root files — read the research records and write Buyers.md, Competitors.md, Problems.md, and Statistics.md, each entry a 1-to-100 rating with its arithmetic, reasoning, and record citations. TRIGGER when research records are ready to be rated into the workspace's judged root files, or when the chief dispatches the judging step. DO NOT TRIGGER to re-derive a project's own judged files from those records (plan-copy) or to gather the evidence itself (the research skills).
---

# Judge Research

One Process: read the research records the research phase produced, and write the four judged root files — `Buyers.md`, `Competitors.md`, `Problems.md`, `Statistics.md`. One data type per file, every entry a rating from 1 to 100 with its arithmetic shown and its citations to the records it rests on. This is the dedicated step between research and strategy: research writes records only, this step rates them, and the strategist reads the rated files. Nothing here is true or false — every rating rides to the owner's pick and nothing is auto-killed for a low number.

## 1. Read the records for each type

Read the records under `research/<subject>/`, sorted by the type each judged file holds. Buyer-quote records (V) and buyer-group evidence feed `Buyers.md`; competitor records (C) feed `Competitors.md`; problem records feed `Problems.md`; statistics records (S) feed `Statistics.md`. Read the whole record, never a summary of it — the rating is computed from what the record actually carries.

### Rate only what a record evidences
Every entry you write cites the record IDs behind it. An entry with no citing record is not written. Owner-stated facts (O) are first-class evidence and enter judged the same way, rated against the rest, never seeded as settled.

## 2. Compute every rating by the grading standard

Each rating is COMPUTED from countables by the grading skill (see grading) — the evidence elements present, the venue and occurrence counts, the quote form, and the per-type ladders for a buyer group, a problem, and the market measurements. Grading is the one home for the arithmetic; reference it, never restate it here.

### Show the arithmetic beside every rating
Every entry states its rating with the arithmetic that produced it and the reasoning in one line, per grading. A bare number with no arithmetic is not graded and does not ship into the file.

### Rate each problem against each buyer group
`Problems.md` carries one rated row per problem-by-buyer pairing — the problem in the buyer's words, the buyer group it is rated against, the record that evidences it, its grading, and its frequency and heat for that group. A problem is rated against every group, never one problem for one buyer. A problem no product record addresses is rated unaddressed, not dropped.

## 3. Write the four judged files

Write each file with one data type and its rated entries: `Buyers.md` the buyer hypotheses, `Competitors.md` the competitors, `Problems.md` the problem-by-buyer pairings, `Statistics.md` industry statistics alone. Each entry carries its rating, its arithmetic, its reasoning, and its record citations. You write these four files and nothing else — never a research record and never copy.

Verification: the records read whole and sorted by type; every entry in the four files carrying a 1-to-100 rating with its arithmetic and its citing record IDs; every problem rated against every buyer group with unaddressed problems kept; nothing auto-removed for a low rating and no verdict status written.
