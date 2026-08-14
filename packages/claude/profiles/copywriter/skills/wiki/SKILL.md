---
name: wiki
description: Use the knowledge wiki — find its root, search before reading, read a topic and its Claude.md, reference material by relative path, re-index after writing. TRIGGER whenever a task needs something the wiki might hold, or writes anything into it. DO NOT TRIGGER to file a session's loose captures (that is journal) or to consolidate topics (that is dream).
---

# Wiki

One Process: touch the wiki through its index, not by crawling the tree. The index is `scripts/wiki.py` at the wiki root; `WIKI_PATH` names the root, default `/Users/jordan/Developer/wiki`.

## 1. Find the root

`$WIKI_PATH`, else `/Users/jordan/Developer/wiki`.

IF `scripts/wiki.py` is absent at the root:
### Fall back to reading Claude.md files directly
The wiki is a topic tree of folders, each with a `Claude.md` summarizing the topic and captures beside it. Read the root `Claude.md`, then the topic folder's `Claude.md`, then the captures. Skip every step below that calls `wiki.py`.

## 2. Search before reading the tree

Run `python3 scripts/wiki.py search "<terms>"` first. It returns the topics and captures that match, so you read those instead of walking folders.

## 3. Read a topic

`python3 scripts/wiki.py topic <name>` for its captures, and read the topic folder's `Claude.md` for the consolidated summary. The `Claude.md` is the knowledge; the captures are the evidence under it.

IF you need what changed lately:
### Use recent
`python3 scripts/wiki.py recent` lists the newest captures across topics.

## 4. Reference material by relative path

Cite a capture or resource with a relative markdown link resolved from the wiki root — `[name](topics/<topic>/<file>)` — never an absolute path, because the tree moves and gets shared.

## 5. Re-index after writing

Any write into the wiki ends with `python3 scripts/wiki.py index` so search and the orphan report see it. On a wiki's first use, run `python3 scripts/wiki.py init` once to create the database, then `index` (index errors if init never ran). One `index` run indexes topics and captures together, so a topic created this session is never left unindexed.

In an orchestrated multi-agent run, only the last writer (or the orchestrator) runs `index`; a writer told an index run follows skips it.

Verification: search ran before any tree read; every wiki path in your output is relative; if you wrote, `wiki.py index` ran last unless an index run was reserved for the orchestrator.
