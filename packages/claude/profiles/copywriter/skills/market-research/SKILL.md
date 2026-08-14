---
name: market-research
description: Run the market-research phase for one workspace — sweep every research axis wide, then mine the entries that surfaced into records, closing each thread and the phase under review. Produces the research/ tree the strategy phase later judges. TRIGGER when a workspace needs its market evidence gathered before any strategy or writing runs. DO NOT TRIGGER to research the product itself (product-research), to rate records into judged files (judge-research), or to plan a piece (plan-copy).
---

# Market Research

One Process: gather one workspace's market evidence into `research/<subject>/` threads — discovery lists plus deep records — and close it under review, so the strategy phase has real evidence to judge. This owns the whole phase, not one thread; the record-level and discovery-level work lives in the skills it composes and is never restated here.

## 1. Discover wide before mining deep

Sweep every axis wide first, so the deep passes only mine what discovery surfaced. Run the discovery skills across their axes: discover-problems and discover-market open the field, then discover-audience and discover-competitors draw the buyer groups and the competitive set. Each returns enumeration into `research/<subject>/discovery/`, never judgment.

## 2. Mine the surfaced entries into records

From what discovery surfaced, run the deep passes — research-problem, research-market, research-audience, and research-competitor — each mining one entry into verbatim records with citations. The researcher orchestrates one thread end to end; its assistants do the page work through /browse and /extract, one page at a time.

- Dispatched through codex, `gpt-5.6-luna` at MEDIUM effort is the assistant model — validated at 100% verbatim fidelity across every extraction test. The dispatch carries the assistant's generated instruction artifact inline, never a file pointer.

## 3. Review each thread and the phase

Sweep unjudged URLs through review-source as the source-reviewer meets them — a page is scored once. Close each thread with its own review, then run review-research at the phase close for a per-thread pass or return-for-re-execution verdict. Nothing is auto-killed; a returned thread re-runs.

## Owner boundary

The phase stops at the research boundary. It produces the `research/` tree and nothing downstream — no judged files, no strategy. The owner may review the assembled research before anything downstream runs.

## Verification

Every discovered axis has a `discovery/` list, every surfaced entry has records with citations, every URL in the registry is judged, and review-research returned a verdict per thread.
