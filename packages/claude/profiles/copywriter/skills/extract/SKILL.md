---
name: extract
description: Pull the problem quotes and on-subject facts out of ONE cached page against a named subject — verbatim quotes where a real person voices a problem, plus the concrete facts, praise and features skipped. TRIGGER when the assistant is handed one page to extract. DO NOT TRIGGER to enumerate a field (discover), to mine a whole thread (research), to rate what a page shows (judge-research), or to fetch a page (browse).
---

# Extract

One Process — take the subject and the one page text the dispatch names, and pull two things out of it: the verbatim problem quotes and the concrete on-subject facts. Extraction only — you do not rate the page, judge the source, or decide what it means. The page text comes from the /browse cache; read the cached file the dispatch names.

Inputs: the subject, and the path to the page text file — nothing else.

## 1. Read the cached page against the subject

Read the whole cached page text the dispatch names, holding the subject in mind. Every quote and fact you pull must relate to that subject.

## 2. Extract the quotes and the facts

Extract (1) every VERBATIM quote where a real person describes a PROBLEM, complaint, frustration, limitation, cost, or unmet desire related to the subject — exact words, speaker as shown. Implied problems (built their own tool, switched away, gave up) belong in Quotes, never Facts. (2) Concrete on-subject facts as stated.

EXPLICITLY SKIP praise, feature descriptions, credentials, advice, and anything off-subject.

Copy exact words including punctuation — never trim, never append.

## 3. Hunt once more before calling a page empty

Before concluding a page holds nothing, re-read it once hunting for complaints, costs, switches, and workarounds phrased in passing. A problem voiced sideways — a line about the spreadsheet they still keep, the tool they dropped, the hour it costs every week — is the quote most often missed on the first pass.

## 4. Output the two sections

Output exactly two sections:

    ## Quotes
    - quote — speaker
    - quote — speaker

    ## Facts
    - concrete on-subject fact as stated

An empty section is honest — write the heading with nothing under it rather than inventing a quote or a fact to fill it.

Verification: every quote is a real person voicing a problem, complaint, cost, limitation, or unmet desire on the subject, copied word-for-word with its speaker as shown and nothing trimmed or appended; implied problems sit in Quotes, not Facts; praise, features, credentials, advice, and off-subject material are absent; the empty-page re-read ran; and the output is the two sections `## Quotes` and `## Facts` and nothing else.
