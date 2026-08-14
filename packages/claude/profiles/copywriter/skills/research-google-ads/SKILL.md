---
name: research-google-ads
description: Run the research pass on one competitor's Google paid creative — its ads from the Google Ads Transparency Center, literal quoted hooks and claims with run dates — into the competitor thread. TRIGGER when a job needs a competitor's Search, Display, or YouTube ads captured. DO NOT TRIGGER for Meta ads (research-meta-ads), TikTok ads (research-tiktok-ads), or a competitor's own site pages (research-competitor).
---

# Research Google Ads

Run /research on one competitor's Google paid creative — one library, the Google Ads Transparency Center. Subject: the competitor thread, `research/competitors/<competitor>/`. research owns the verbatim record format with full citations and the leave-unjudged rule. This skill adds only the Google-ads axis below.

Inputs: the one named competitor and the Google Ads Transparency Center — nothing else. The thread is blind to our product: no product name, no product files, no our-side axes.

## Read the library through agent-browser

The Transparency Center is interactive, so read it through agent-browser (`agent-browser skills get core`), never WebFetch or WebSearch. Close the session before the step ends.

- URL: `https://adstransparency.google.com/`
- Search the competitor's advertiser or domain.
- Clear the geo filter to Anywhere — the default region hides ads served elsewhere and returns a thin, misleading set.
- Extract from the advertiser overview page, not the individual creative-detail pages — the overview lists the advertiser's ads with their formats and last-shown date ranges; the detail pages are slow and add nothing to the record.
- Pace requests. A burst trips a 429; it recovers after about 60 seconds, so wait and continue rather than abandoning the sweep.

## Record what the ad says and how long it ran

Capture the ad's literal hook and claim verbatim, quoted — for Search text ads the headline set and description lines, for YouTube the first-five-seconds hook and the offer — and its last-shown date range as observed facts. Never infer the angle it plays, never label an ad a "winner", never read what converts. What the ad SAYS is the record; why it works is the judged-files step's judgment. The center shows creative and date ranges only — never claim spend, keywords, or performance from it.

Write the records into `research/competitors/<competitor>/ads-google.md` — each record carrying the verbatim quoted hook and claim, its format, its last-shown date range, its library URL, and the date you captured it, inline. Left unjudged.
