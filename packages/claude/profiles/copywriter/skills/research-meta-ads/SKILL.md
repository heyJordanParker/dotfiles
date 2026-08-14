---
name: research-meta-ads
description: Run the research pass on one competitor's Meta paid creative — its live and past ads from the Meta Ad Library, literal quoted hooks and claims with run dates — into the competitor thread. TRIGGER when a job needs a competitor's Facebook or Instagram ads captured. DO NOT TRIGGER for Google ads (research-google-ads), TikTok ads (research-tiktok-ads), or a competitor's own site pages (research-competitor).
---

# Research Meta Ads

Run /research on one competitor's Meta paid creative — one library, the Meta Ad Library. Subject: the competitor thread, `research/competitors/<competitor>/`. research owns the verbatim record format with full citations and the leave-unjudged rule. This skill adds only the Meta-ads axis below.

Inputs: the one named competitor and the Meta Ad Library — nothing else. The thread is blind to our product: no product name, no product files, no our-side axes.

## Read the library through agent-browser

The Meta Ad Library is interactive, so read it through agent-browser (`agent-browser skills get core`), never WebFetch or WebSearch. Close the session before the step ends.

- URL: `https://www.facebook.com/ads/library/`
- Set country and category to "All ads", then search the competitor's exact page name or brand.
- Each ad shows a "Library ID" and a start date. Filter by "Active" for what runs now; drop the filter for the full history of tested creative.
- Coverage is by page, not product — a large advertiser mixes many offers, so read which product each ad points to before filing it.
- This library works clean: search returns the page's ads directly.

## Record what the ad says and how long it ran

Capture the ad's literal hook and claim verbatim, quoted, and its run dates — start date, and Active or last-seen — as observed facts. Never infer the angle it plays, never label an ad a "winner", never read what converts. What the ad SAYS is the record; why it works is the judged-files step's judgment. The library shows creative and dates only — never claim spend, targeting, or performance from it.

Write the records into `research/competitors/<competitor>/ads-meta.md` — each record carrying the verbatim quoted hook and claim, its run dates, its Library ID and library URL, and the date you captured it, inline. Left unjudged.
