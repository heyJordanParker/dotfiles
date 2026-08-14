---
name: review-research
description: The research-phase review — read the assembled research against the research SOP and return a per-thread verdict of pass or return-for-re-execution with the named gap. TRIGGER when the chief closes the research phase and needs the assembled threads reviewed before the judged files are written. DO NOT TRIGGER to rate the records into judged files (judge-research) or to attack whether the claimed market exists (check-reality).
---

# Review Research

One Process: read the assembled research phase — the commissioning `Brief.md`, the chief's dispatch record, and the `research/` tree — and return one verdict per thread against the SOP below. You review the artifacts; you never rewrite a record or a discovery list. Each verdict is pass or return-for-re-execution with the one named gap, and the findings go to the chief, who re-commissions the returned thread. Nothing is auto-killed and no verdict status is written into the research tree.

## 1. Read the phase against its inputs

Read the commissioning `Brief.md` for the starting domains and the owner's problems, the chief's dispatch record for the threads that ran, and the `research/` tree for what each thread produced. A thread the brief's domains call for that never ran is itself a gap.

## 2. Check the four axes have real discovery per starting domain

For each starting domain in the brief, confirm the four axes — audience, problems, competitors, product — carry threads whose `discovery/` lists hold real enumeration, not a thin stub. An axis with no discovery, or discovery that names one entry where the field holds many, is a gap: return the thread naming the axis and the domain it is thin on.

## 3. Check competitor deep threads are single-subject

Each competitor research thread covers ONE competitor. A thread that folds two competitors into one file, or reaches another thread's subject, is a gap: return it naming the subjects that must split.

## 4. Spot-check that records trace to fetched pages

Sample records across the threads and confirm each traces to a page that was actually fetched, re-reading the cited URL through the read gate:

    python3 <profile>/skills/browse/scripts/browse.py <url>

Confirm the record's verbatim words appear in the page text the gate returns. A record whose words are not on its cited page, or whose "source" is a search summary or snippet rather than a fetched page, is search-summary material in the records — return the thread naming the record. A record labeled PARAPHRASE is honest; a paraphrase presented as a verbatim quote is the gap.

## 5. Check buyer voices are present in the problem threads

Each problem thread carries buyers' own verbatim words for the problem, not a paraphrase of the pain in professional terms. A problem thread with no buyer voice is a gap: return it.

## 6. Check the category's obvious market leaders appear in competitor discovery

The competitor discovery for a category names the market leaders anyone in the category would list. When an obvious leader is absent, the discovery swept too narrow: return the competitor thread naming the missing leader.

## 7. Check every thread is encapsulated

Each thread reads only its own inputs and references no other thread's files: the market and competitor threads run blind to each other, and the product thread runs isolated from both. A record that cites, quotes, or is shaped by a sibling thread's file breached its isolation — return the thread naming the breach.

## 8. Return one verdict per thread

For each thread, return pass or return-for-re-execution with the one named gap and the check it failed. Hand the findings to the chief; you write nothing into the research tree and you rewrite no artifact.

Verification: every thread the brief's domains call for carries a verdict; each return names one gap and the check it failed; the four axes, single-subject competitor threads, fetched-page provenance, buyer voices, market-leader coverage, and encapsulation are each checked; sampled records were re-read through the page gate and their words confirmed on the page; and no verdict status or edit was written into the research tree.
