---
name: journal
description: File a session's loose material into the knowledge wiki — each capture into its owning topic folder, the topic's Claude.md kept current. TRIGGER after a session hands you loose captures (notes, clippings, findings, screenshots) that need a home in the wiki. DO NOT TRIGGER to gather new research, or to consolidate the wiki itself (that is dream).
---

# Journal

One Process: a session's material gets a home in the wiki's topic tree and the topic's Claude.md stays current. The wiki root comes from the invocation or the calling context. A dispatched filer cannot ask: if the root is missing, escalate it to the dispatcher as a blocking gap and return the captures unfiled — never complete the task with reusable findings left unfiled.

Run fresh: work only from the capture paths and origins handed to you, never the dispatching session's conclusions.

## 1. Find the owning topic

The tree has four branches: `topics/copywriting/` for craft, `topics/markets/<market>/` for market findings, `topics/companies/<name>/` for competitor research, and `topics/products/<product>/` for product-specific material. Route each capture to its branch. A product-only capture that does not generalize does not belong in the wiki at all — it stays in that product's `research/`.

Prefer an existing topic. Create a topic folder only when nothing owns the capture, name it in the plain words a person would say, and name the creation in your report.

## 2. Write the capture into the topic

A dated markdown file noting where it came from. Images and binaries sit beside it, referenced relatively.

When the source's origin date and the filing date differ, the capture carries both as plain lines at the top: origin `YYYY-MM-DD` and filed `YYYY-MM-DD`.

Link between topics and captures with relative markdown links resolved from the wiki root — `[name](topics/<topic>/<file>)` — never absolute paths.

## 3. Keep the topic's Claude.md current

Fold the capture's substance in. Never overwrite material that contradicts it — note the contradiction instead.

## 4. Re-index

`python3 scripts/wiki.py index` at the wiki root so search and the orphan report see the new captures. On a wiki's first use, run `python3 scripts/wiki.py init` once before the first `index` (index errors without the database). The same `index` run picks up any topic folders you created this session alongside their captures, so a new topic never sits unindexed.

In an orchestrated multi-agent run, only the last writer (or the orchestrator) runs `index`; a filing agent told an index run follows skips this step.

## 5. Report

What was filed where, topics created, contradictions noted.

Verification: every capture is on disk under a topic, every touched topic's Claude.md mentions it, nothing that was there before is gone, and `wiki.py index` ran last unless an index run was reserved for the orchestrator.
