---
name: research-competitor
description: Orchestrate one competitor's research thread — dispatch single-task agents to capture its landing page, its user reviews, the problems it solves, the market it serves, and its offer, all from its own surfaces — into the competitor thread. TRIGGER when one named competitor needs its thread researched. DO NOT TRIGGER to draw the competitive set (discover-competitors), to mine where buyers talk (discover-audience), to research one problem (research-problem), or to pull ads (research-meta-ads, research-google-ads, research-tiktok-ads).
---

# Research Competitor

Run /research-thread for ONE competitor — that skill owns the orchestration shape (scaffold, single-task dispatches, file yourself, close under review); this adds only the competitor axis: the fixed capture set, the product-blind law, and the thread folder `research/competitors/<competitor>/`, one topic file per capture. Each capture runs /research; the orchestrator captures nothing itself.

The thread is COMPLETELY blind to our product. Every dispatch carries the competitor and its one task — never our product name, our product files, our `Brief.md`, our buyers, or our axes. The competitor is evaluated on its own merits, from its own surfaces. A dispatch that leaks our product biases the agent into confirming our frame instead of recording theirs.

Threads never reference other threads' files: no dispatch names the market thread's discovery lists or quotes, and no capture reads them.

## 1. Scaffold the thread folder

Create `research/competitors/<competitor>/` before dispatching. If it exists, read its topic files first and dispatch only the captures still missing or thin — never re-derive a capture the thread already holds.

## 2. Dispatch one agent per capture, in parallel

Dispatch five single-task agents at once, one per capture below, per /research-thread's dispatch shape. Each dispatch carries only the competitor's name and its one task, nothing from a sibling capture. Each agent runs /research, RETURNS its capture, and leaves its records unjudged; the thread orchestrator files every return at its topic path itself, trailers stripped — assistants never write the thread's files.

### a. Capture the landing page
Read the competitor's landing page through browse for its full visible copy, written verbatim into `landing.md`, AND capture a full-page screenshot through screenshot into the thread folder, the screenshot filename recorded in `landing.md`. A company landing page rarely walls, so both normally succeed; if a bot wall blocks the screenshot, keep the browse text capture and record the gap.

### b. Mine verbatim user reviews
Mine real users' own words about the competitor from review platforms — the words of named reviewers, quoted verbatim, never a summary or a paraphrase of the sentiment. Read each review page through /browse, or agent-browser where the platform is interactive. Write the quotes into `reviews.md`, each carrying its reviewer identity as shown, its URL, and its dates. Review platforms and forums are primary surfaces; a listicle or SEO blog ABOUT the competitor is never a review source.

### c. Derive the problems it solves
Derive the problems this competitor solves independently from ITS OWN surfaces — its pages, its claims, its own words — never from any problem statement we hold. Read the surfaces through the page gate. Write into `problems.md` the problems the competitor's surfaces assert it solves, worded as the competitor's claim, each cited to the surface that states it.

### d. Derive the market it serves
Derive who this competitor sells to independently from its own surfaces — who its pages address, in its own words. Read through the page gate. Write into `market.md` the audience its surfaces target, worded as the competitor's own framing, each cited to the surface that states it.

### e. Walk the offer and pricing
Walk the competitor's offer from its own pages — outcome promised, named mechanism, deliverables, terms, price, risk reversal — reading each page through the page gate or agent-browser where interactive. Write into `offer.md` the offer exactly as the competitor presents it, verbatim on price and terms, each cited to the page that shows it.

### A competitor's claim is a record that it makes the claim
What a competitor asserts on its surfaces — a problem it solves, a market it serves, a wedge it stakes — is a record that the competitor SAYS this, never that it is true or that the buyers it targets exist. Never convert a surface claim into a proven buyer market or real demand. A wedge with no buyer evidence behind it is the competitor's bet, worded that way.

## 3. Review the closed thread against this SOP

Before closing, review the finished thread and return any failing capture to a fresh single-task agent for redo:

- Provenance: every record traces to a page the registry logged — spot-check URLs through /browse's registry check. A record with no logged page is not a fetched record.
- Source rules: reviews come from real reviewers on primary platforms, quoted verbatim; no listicle or SEO blog stands as a research source; no record rests on a search summary.
- Encapsulation: no capture read our product, our files, or a sibling thread's files; problems and market were derived from the competitor's own surfaces only.
- One task per agent: each topic file is one capture; `landing.md` carries both the verbatim copy and the screenshot filename.

Verification: the thread folder `research/competitors/<competitor>/` holds `landing.md` (verbatim copy plus a saved screenshot), `reviews.md` (verbatim named-reviewer quotes), `problems.md`, `market.md`, and `offer.md`, each capture written by its own single-task agent; every record is fetched-verbatim or labeled PARAPHRASE with its URL, speaker, and dates; problems and market were derived from the competitor's own surfaces, blind to our product; the thread references no other thread's files; the records are left unjudged; and the closing review passed with any failing capture redone.
