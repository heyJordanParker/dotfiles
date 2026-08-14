---
name: discover
description: The broad pass of a research thread — take one subject, sweep wide, and return long lists of what exists with one line of observed context per entry, into research/<subject>/discovery/. Enumeration, never judgment. TRIGGER when a thread needs its field enumerated before any one entry is studied, or when a discovery specialization names this as its Process. DO NOT TRIGGER to study one entry deep (research) or to rate what discovery found (judge-research).
---

# Discover

One Process — the broad pass: take the subject the dispatch names, sweep its field wide, and return long lists of what exists into `research/<subject>/discovery/`, one line of observed context per entry. This returns the lists, not the study; a research pass mines each entry into records afterward. Enumeration is the whole job — stop at breadth, never study one entry deep here.

Inputs: the one subject, named by its question, and whatever the dispatch's specialization names as the field to sweep — nothing else. The dispatch carries the subject only: no prior findings, no phrasing from another thread, no reference to a sibling thread, and no knowledge of what a later step does with the lists. If the thread folder `research/<subject>/` is missing, escalate it to the dispatcher as a blocking gap and stop — never complete the task with the lists unwritten.

## 1. Read the thread first

Read `research/<subject>/discovery/` for entries already found. Add only new ones; never re-derive an entry the lists already hold.

## 2. Sweep the field wide

Answer the subject's question long — enumerate every entry the field holds, one line of context per entry. Reach for breadth over depth: many entries with a line each, never a few studied closely. The dispatch's specialization names the entry types to sweep and the surfaces to reach them through.

- Codex native search is the discovery search path — the one path that surfaces the community layer (Reddit, forums) where buyers voice problems in their own words. HN Algolia (`hn.algolia.com/api/v1/search`) is the one supplementary API worth a `curl` on top of it.

## 3. Record observed facts per entry, never verdicts

### Read every page through /browse
/browse owns the trust check, the fetch, the cleaning, the caching, and the registry logging. A nonzero exit means the page could not be read and the registry already records it — move to the next entry, never guess its contents.

### Find with search and listicles, record only from the fetched page
Search results, listicles, and SEO round-up posts are legal for FINDING candidate names and venues — the discovery job is to surface what exists. They are never a content source: every countable fact, number, and quoted word in an entry is read off the entry's own page fetched through /browse, and each entry cites the URL where it was observed. A search snippet may supply the URL, never the entry's words — an entry whose page did not fetch carries its URL and the recorded outcome, no content.
Never: an entry quoting a speaker's words from a search snippet, or an entry with content for a page /browse exited nonzero on.

### Record countable facts, never a verdict
Per entry, record its URL, what it is in one line, and countable facts read off the page — counts, activity, recency. No richness, quality, or position verdict, and no quote or record extraction — those are the research pass's, dispatched after the thread.

## 4. Write the lists to the discovery subfolder

Write the discovered entries into the contract path as long lists, one line of context per entry, readable. Each entry carries its URL. Records — the deep study of any one entry — are the research pass's output into the thread's topic files, never written here. No index file, no judgments.

### Write to the contract path and nowhere else
The lists live at exactly one path. Write them there, then read that path back to confirm they landed — a deliverable that is not at its contract path does not exist, and a byte-identical second copy anywhere else is not a second deliverable, it is a filing violation. The contract path is the file's only home; never also emit the lists at the run root or under a name of your own.
Never: `discovery-<subject>-output.md`, `<field>-output.md`, or any twin of the file at the run root — the duplicate beside the run root is not a backup, it is stranded output.

Template:
    research/<subject>/discovery/<field>.md

### File the deliverable under research, never under working state
`.agents/` holds working state — codex-run output, wrapper trailers, scratch. The lists are done only when they live under `research/<subject>/discovery/`; nothing is done while its output sits under `.agents/`, and the codex-run trailer never travels into the deliverable.

### Enumerate only, reference no other thread
The output is enumeration and nothing more. No recommendation, no "this leads to" or "worth a deep dive" nomination, no gap-spotting synthesis, no forward section about what a later step should do. Never name, quote, or link another thread's files — a thread's lists stand alone.

Verification:
- The lists enumerate the field wide; each entry carries a URL and a one-line observed context.
- Every entry's content comes from a page /browse fetched; an unfetched page's entry carries the URL and outcome only.
- No entry carries a recommendation or a reference to another thread.
- The lists live at `research/<subject>/discovery/<field>.md`, no index, no judgments, read back from that path after writing.
- No deliverable sits under `.agents/`; no duplicate copy sits at the run root or under a name of your own.
- You read nothing the Inputs section excludes.
