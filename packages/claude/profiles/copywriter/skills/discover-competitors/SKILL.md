---
name: discover-competitors
description: Competitor discovery — run the discover pass on the competitor axis: everyone competing for this buyer's money, drawn two ways (same-problem and same-audience). TRIGGER when the competitor thread needs its set enumerated before any one is studied. DO NOT TRIGGER to study one competitor's own surfaces (research-competitor), to find where buyers talk (discover-audience), or to pull a competitor's ads (research-meta-ads, research-google-ads, research-tiktok-ads).
---

# Discover Competitors

Run /discover on the competitor axis. Subject: the competitor thread. discover owns the sweep, the page gate, the trust check, the observed-facts-only rule, and the write into `research/<subject>/discovery/`. This skill adds only the competitor axis below.

Inputs: the problem statements from the commissioning dispatch — nothing else. Excluded: `product/Brief.md`, the product name, product facts, our own axes, and any other thread's files. The dispatch carries the problem statements and nothing more; you draw the whole set from those alone, blind to our product and blind to sibling threads.

## Draw the set two ways

Enumerate two lists, each answered long, one line of observed context per entry:

- **Same-problem** — products a web search surfaces solving the problems the dispatch names, that a buyer could buy. Include the three kinds the disposition forgets: direct rivals in the same category, indirect ones in a different category doing the same job (the spreadsheet, the agency, the manual workaround), and do-nothing — the buyer keeps the status quo, the most common competitor and the easiest to miss.
- **Same-audience** — products that sell to the same people the same-problem set serves, whatever problem they solve. What else that population already pays for is who competes for the same wallet and attention. Draw this population from the buyers the same-problem search surfaces, never from a sibling thread's files.

### Sweep for the category's biggest players
Run the obvious "biggest X tools" and "most popular X" searches explicitly, so the category's largest and most obvious players land on the list. The disposition drifts to niche or novel finds and skips the market leaders a buyer already knows; naming them is required, not optional.

Per competitor, record which list it came from, its URL, the headline claim its page states in its own words, and the price or offer the page shows. Web search is URL-finding only; the recorded facts come from the page read through the gate.

## Enumerate only — no synthesis

Discovery is enumeration. Do not write what a competitor "competes on", do not nominate deep-research candidates, do not note gaps, do not synthesize across the two lists. What each competitor competes on and whether the set forms a real market are the judged-files step's reads, from the research that follows.

### A drawn competitor is a record, not a proven market
Finding companies in a category records that the companies exist, never that a real buyer market exists — finding attribution companies does not prove buyers want attribution. The set is a list of who competes; whether it is a real market is settled later.

Write both lists into the dispatched subject's own thread, `research/<domain-slug>-competitors/discovery/` — the same-problem list and the same-audience list, each entry carrying its URL and its one-line observed context. `research/competitors/<competitor>/` is the DEEP single-competitor thread's home (research-competitor), never a discovery destination — two domains discovering competitors into one shared folder collide.

Verification: both lists enumerate the field wide, the same-problem list drawn from the dispatch's problem statements alone and the same-audience list from the buyers those competitors serve, blind to `product/Brief.md`, the product name, and every other thread's files; the biggest-players sweep was run so market leaders appear; each entry carries a URL and one line of observed context; the lists hold no synthesis, no leads, and no judgments; and they live in the subject's own `research/<domain-slug>-competitors/discovery/`.
