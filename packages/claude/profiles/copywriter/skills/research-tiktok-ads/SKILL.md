---
name: research-tiktok-ads
description: Run the research pass on one competitor's TikTok paid creative — its ads from the TikTok Commercial Content Library, literal quoted hooks and claims with run dates — into the competitor thread. TRIGGER when a job needs a competitor's TikTok ads captured. DO NOT TRIGGER for Meta ads (research-meta-ads), Google ads (research-google-ads), or a competitor's own site pages (research-competitor).
---

# Research TikTok Ads

Run /research on one competitor's TikTok paid creative — one library, the TikTok Commercial Content Library. Subject: the competitor thread, `research/competitors/<competitor>/`. research owns the verbatim record format with full citations and the leave-unjudged rule. This skill adds only the TikTok-ads axis below.

Inputs: the one named competitor and the TikTok Commercial Content Library — nothing else. The thread is blind to our product: no product name, no product files, no our-side axes.

## Read the library through agent-browser

The library is interactive, so read it through agent-browser (`agent-browser skills get core`), never WebFetch or WebSearch. Close the session before the step ends.

- URL: `https://library.tiktok.com/ads`
- The index is EU-only and matches on keyword, not on a verified brand — a search returns ads whose text mentions the competitor, not a confirmed set of that brand's ads. Read each result and keep only the ones whose advertiser is the competitor.
- Sweep the date range month by month. A single wide window truncates results; stepping one month at a time surfaces the full run of tested creative.
- Budget long runs. The month-by-month sweep across the EU index is slow — plan for a long session rather than cutting it short.

## Record what the ad says and how long it ran

Capture the ad's literal hook and claim verbatim, quoted, and its run dates — first-shown and last-shown from the library — as observed facts. Never infer the angle it plays, never label an ad a "winner", never read what converts. What the ad SAYS is the record; why it works is the judged-files step's judgment. The library shows creative and dates only — never claim spend, targeting, or performance from it.

Write the records into `research/competitors/<competitor>/ads-tiktok.md` — each record carrying the verbatim quoted hook and claim, its run dates, its library URL, and the date you captured it, inline. Left unjudged.
