---
name: research
description: The deep pass of a research thread — take one subject, mine it deep, and write verbatim records into research/<subject>/<topic>.md with full citations. Records only, never judgment. TRIGGER when one discovered entry needs its records extracted, or when a research specialization names this as its Process. DO NOT TRIGGER to enumerate a field broad (discover) or to rate the records (judge-research).
---

# Research

One Process — the deep pass: take the one subject the dispatch names, mine it deep, and write the records into `research/<subject>/<topic>.md`. Records only — no rating of what the records show; that judgment is the judged-files step's, from these records.

Inputs: the one subject, and whatever the dispatch's specialization names as its capture target — nothing else. If the thread folder `research/<subject>/` is missing, escalate it to the dispatcher as a blocking gap and stop — never complete the task with the records unwritten.

## 1. Read the thread first

Read the dispatched thread folder `research/<subject>/` for what this subject has already yielded. Extract only the delta.

## 2. Read the subject through /browse

/browse owns the trust check, the fetch, the cleaning, the caching, and the registry logging. Deprioritize a low-trust domain by reading it more skeptically, never by auto-skipping it. A nonzero exit means the page could not be read and the registry already records it — move on; the unreadable page is recorded, never guessed. Reserve agent-browser for genuinely interactive work and close the session before the step ends.

## 3. Capture verbatim records

Every record comes from a page you fetched through /browse — nothing else is a record. A record is verbatim when you copied the words from that fetched page. PARAPHRASE is the narrow case where you fetched the page but the exact quote could not be cleanly extracted (a scanned image, a mangled render): you still read the page, so you write your closest rendering and label it PARAPHRASE. Search-summary and snippet text is never a record and never a PARAPHRASE — it is discovery material for finding the page, and if you cannot fetch the page, there is no record. Every record carries the full URL, the speaker's identity as shown on the page, the source's Published and/or Updated date, and the date you captured it, inline. Numbers about the source itself — review counts, ratings, member counts — are read from the fetched page, never from a search summary.

### Ban listicle and SEO-blog domains as record sources
A listicle, round-up, or SEO-blog page is discovery material for finding primary surfaces, never a record source. Records come from primary surfaces — review platforms, forums, communities, and the vendor's own pages for the vendor's own claims. Reading a round-up to find where the buyers talk is fine; quoting the round-up as a record is not.

Capture the exact words. "We were drowning in spreadsheets" is the record; "manual-process inefficiency" destroys it. Never invent a record to fill a category — an empty category is an honest record. The dispatch's specialization names what to capture and how to sort it.

## 4. Leave the subject unjudged

You do not score the subject. Reading it through browse already logged its URL to the registry as unjudged; source quality is the source reviewer's, dispatched by the chief after the thread.

## 5. Write the records into the thread

Write the records into the dispatched thread's topic files — one file per topic, records only, readable, each record carrying its verbatim words or PARAPHRASE label, speaker, full URL, the source's Published and/or Updated date, and the date you captured it, inline. No index file, no judgments — the topic file holds the raw records and nothing else. Never name, quote, or link another thread's files — this thread's records stand alone.

### Write to the contract path and nowhere else
Each topic's records live at exactly one path. Write them there, then read that path back to confirm they landed — a record that is not at its contract path does not exist, and a copy anywhere else is not a second record, it is a filing violation. The topic file is the record's only home; never also emit the extracted quotes and facts at the run root or under a working name of your own.
Never: `extract-<subject>-<source>.md`, `<topic>-output.md`, or any working copy of a record at the run root — the extract is filed into `research/<subject>/<topic>.md` and left nowhere else.

Template:
    research/<subject>/<topic>.md

### File records under research, never under working state
`.agents/` holds working state — codex-run output, wrapper trailers, scratch. A record is filed only when it lives under `research/<subject>/<topic>.md`; nothing is done while a record sits under `.agents/`.

### File only on-subject extracts
Before filing an extract, confirm its quotes are about this subject. Return an extract whose quotes are about a different topic to the assistant with the subject restated, unfiled. When a page is genuinely about the other topic, note its URL off-subject in the source registry (`sources.py log <url> --note`) and file no record from that page. Page outcomes — unreadable, off-subject, render caveats — live in the registry only, never as a file in the thread: the records are the deliverable, the registry is the implementation ledger.
Never: a `discovery/` folder or a page-outcomes file inside a deep thread — `discovery/` is the discover pass's output alone.

### State in the file why a record is thin
A record below 3 entries states in the file itself why it is thin — the page was thin, off-subject, or paywalled. The record body is the extract's Quotes and Facts content alone; codex-run trailers, session or model lines, and other tool output never enter a record.

## 6. Review the thread against this contract before it closes

The thread orchestrator runs this closing review before the thread closes — reviewing the extracts its assistants returned, and holding any record it filed itself to the same checks. Re-read the written records and check them against this contract: every record's URL resolves through a /browse registry lookup proving the page was fetched through the gate; no record traces to a search summary, a listicle, or an SEO-blog domain; nothing references another thread's files; and a discovery thread's output is enumeration only. Return any failing record to the researcher for redo — a breached record is rejected, never folded in.

Verification:
- Every record is fetched-verbatim or labeled PARAPHRASE, each with its full URL, speaker, source date, and capture date.
- No record traces to a search summary, listicle, or SEO-blog domain; unreadable pages are recorded, never guessed.
- The subject is left unjudged; no record references another thread's files.
- The records live at `research/<subject>/<topic>.md`, no index, no judgments, read back from that path after writing.
- No record sits under `.agents/`; no working copy sits at the run root or under a name of your own.
- Every filed extract is on-subject; an off-subject page is noted in the registry with no record filed, and the thread holds no `discovery/` folder and no page-outcomes file.
- Every record below 3 entries states in the file why it is thin and carries no codex-run trailer or tool output.
- The closing review ran against this contract and failing work was returned.
- You read nothing the Inputs section excludes.
