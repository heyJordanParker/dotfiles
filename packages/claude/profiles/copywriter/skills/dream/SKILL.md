---
name: dream
description: Consolidate the knowledge wiki — revisit journaled topics, fold repeated findings into each topic's Claude.md, surface contradictions, keep cross-links true. TRIGGER on a scheduled pass or when a topic tree has grown messy or redundant. DO NOT TRIGGER to file fresh session material (that is journal), or to gather new research.
---

# Dream

One Process: what journal filed gets consolidated into knowledge. Captures are evidence and are never deleted; only topic summaries move. The wiki root comes from the invocation or the calling context; ask when missing.

Run fresh: work only from the filed captures and their origins, never the dispatching session's conclusions.

## 1. Walk the scoped topics

`python3 scripts/wiki.py index` at the wiki root, and walk its orphan and unresolved-link report as the list — the whole tree, or the subtree given. The tree has four branches: `topics/copywriting/`, `topics/markets/`, `topics/companies/`, and `topics/products/<product>/`; product topics live under `products/` so the main folder never balloons. In an orchestrated multi-agent run, only the last writer (or the orchestrator) runs `index`; when told an index run follows, walk the filesystem subtree instead.

## 2. Consolidate each topic's Claude.md

Fold repeated findings together. Note contradictions with both sides and their sources — resolving them is the owner's, not yours.

Every topic's Claude.md opens with a 2-3 sentence summary paragraph, then a "Children" list — one line per sub-topic, its micro-summary and its relative link. Maintain both on every pass.

## 3. Keep cross-links true

Fix links that moved; flag links to topics that no longer exist. Links are relative markdown resolved from the wiki root — `[name](topics/<topic>/<file>)` — never absolute paths.

## 4. Report

Per topic: what consolidated, contradictions surfaced, links fixed.

Verification: no capture file was deleted or edited; every touched Claude.md is shorter or clearer than before; every link in touched topics resolves.
