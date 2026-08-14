---
name: research-thread
description: Orchestrate ONE research thread end to end — take the dispatched kind (discover or research) and subject, run the axis's pass with one assistant per search or page, file the results, and close under review. TRIGGER when the researcher is handed one thread to run. DO NOT TRIGGER to sweep one field yourself (discover), to mine one entry yourself (research), to extract one page (extract), or to run a whole phase (market-research).
---

# Research Thread

One Process — orchestrate one thread. The dispatch carries the workspace root, the KIND (discover or research), and the ONE subject; the axis skill matching the kind and subject owns what to capture, /discover and /research own the record contracts, and /extract owns the page work. This skill only sequences them — it restates nothing they own, and the orchestrator captures nothing itself in volume.

Inputs: the workspace root, the kind, and the subject — nothing else. The axis skill's own Inputs and exclusions bind every dispatch you make.

## 1. Scaffold and read the thread

Create `research/<subject>/` if missing, naming the folder with a short kebab-case slug of the subject — never the subject's full question as a folder name. Read what the thread already holds and work only the delta — never re-derive what is already filed. You name the folder once; every dispatch you make carries that folder path, so no assistant invents its own.
Example: subject "what problems do people voice about funnel attribution?" → `research/funnel-attribution-problems/`.

## 2. Run the pass through single-task assistants

ONE ASSISTANT = ONE TASK. A discovery search is one assistant dispatch carrying the query; a page extract is one assistant dispatch carrying the subject and the cached page path from /browse. Each dispatch carries its one task and the subject only — no sibling results, no thread narrative, no meta context about the run. You read every page through /browse before handing its cache path to an assistant; a page /browse could not read gets no assistant and no content.

## 3. File every return yourself

You file what the assistants return — discovery lists per /discover, records per /research — at the contract paths the axis skill names. An assistant return is a claim: verify it against the cached page before filing, and re-dispatch thin work.

## 4. Close under the contract review

Run the closing review the base pass names — /research's for a deep thread, /discover's Verification for a discovery thread — and return failing work for redo before the thread closes.

Verification: every search and extract ran as one single-task assistant dispatch; every filed entry traces to a page /browse fetched; the lists and records sit at their contract paths and passed the base pass's own Verification; and no dispatch carried a sibling's results or meta context.
