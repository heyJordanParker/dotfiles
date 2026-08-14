---
name: review-source
description: Review ONE research URL for trustworthiness and usefulness (each 1-100) and record its facts into the sources registry — one judgment per URL, reasoning stated. TRIGGER when the source reviewer sweeps an unjudged URL the registry holds. DO NOT TRIGGER to locate sources (discover-audience) or to extract quotes from one (research-problem).
---

# Review Source

One Process: read one page, score its trustworthiness and its usefulness (each 1-100), record its facts, and write the judgment once.

- The registry command is the profile-rooted CLI: `python3 <profile>/scripts/sources.py`.
- A URL is judged once. If it is already judged, stop — do not re-judge without the Architect's `--force` intent.
- The two scores are independent: a page can be honest and empty (high trust, low usefulness) or a rich astroturf pile (low trust, high usefulness).
- Dispatched through codex, `gpt-5.6-luna` at LOW effort is the sweep model — owner-accepted from the 7-rung 10-URL experiment (docs/agents/090-source-review-tiering/): directionally correct at every rung, luna-low included. The dispatch carries this instruction set inline (the generated artifact), never a file pointer.

## 1. Record the URL and its facts

Read the facts off the page and log them in one call. `log` is idempotent and folds in any newly supplied field:

    python3 <profile>/scripts/sources.py log <url> --type "<label>" --published YYYY-MM-DD --updated YYYY-MM-DD --author <id>

- **Type** — a plain label for what the page is: `review site`, `forum thread`, `vendor blog`, `SEO listicle`, `news`, `docs`, `personal blog`, `aggregator`. Not a score.
- **Published / Updated** — fill at least Updated. When the page shows no date, work the ordered extraction methods in [references/finding-dates.md](references/finding-dates.md) (JSON-LD, article meta tags, visible bylines, URL patterns, feeds, sitemaps, Wayback first-capture, platform APIs, Last-Modified last) before leaving a date empty — and never invent one.
- **Author** — link an existing author, or create one when the page names a real byline:

      python3 <profile>/scripts/sources.py author add --first-name "..." --last-name "..." --usernames "handle" --twitter-url "..."
      python3 <profile>/scripts/sources.py author link <url> <author-id>

  Skip the author entirely for a page with no identifiable byline (most listicles, aggregators). Never fabricate a name to fill the field.

### Read the page through browse, and judge only its cached text
Get the page's text by running /browse, which owns the fetch, cleaning, caching, and registry logging:

    python3 <profile>/skills/browse/scripts/browse.py <url>

Judge from the text it prints, nothing else. A nonzero exit IS the stop: the page could not be read, the command already recorded the failure on the row, and there is no judgment path for an unread page — report it and move on. A judgment guessed from the URL or from someone else's description poisons the registry.

## 2. Score trustworthiness

Read the whole page and ask who benefits from it existing, then set one number 1-100. Read down these signals, weigh them together, and state the reasoning in one line — no sub-scores, no per-signal math.

Anchor the scale before weighing: a neutral reference with no motive sits around 90, a nuanced named-user review page around 65-75, a disclosed-motive vendor or affiliate page around 45-60, an empty or abandoned page around 30, astroturf or sourceless filler below 25. Place the page relative to these anchors, then let the signals move it.

Signals that pull the number DOWN:

- **Who profits** — the page exists to sell the thing it praises: vendor's own blog, a page whose only exit is a purchase.
- **Affiliate links** — outbound links tagged for commission; the "best of" ranking follows the payout, not the merit.
- **Astroturf smell** — uniform praise, repeated marketing phrasing across "different" voices, reviews clustered in time, no dissent.
- **AI filler** — generic, padded, sourceless prose that restates the query and commits to nothing.
- **Exaggeration without contrast** — a 1-star or 5-star extreme that only loves or only hates discounts the page; it is a mood, not a report.

Signals that pull the number UP:

- **Nuance** — a 2-to-4-star review that names what worked AND what did not; graded, specific, weighing tradeoffs. This is the most trustworthy shape.
- **Concrete events** — claims tied to a specific thing that happened, dated, attributable — not a floating adjective.

## 3. Score usefulness

Set one number 1-100 for how much usable buyer material the page carries for a copywriter mining language and evidence. State the reasoning in one line.

Count what is actually on the page:

- **Verbatim language** — the buyer's own words for the pain, the desire, the objection.
- **Concrete events** — specific things that happened a piece of copy could stand on.
- **Numbers** — prices paid, counts, durations, results the buyer names.

The page's TYPE is a signal in how you weigh these, not a cap on the score: verbatim buyer speech tied to a real purchase carries the richest usable material, aggregated review stats show direction and prevalence but hand you no sentence to run, a competitor's own self-descriptions show what the category claims but not what a buyer believes, methodical press gives category sizing but not the buyer's words, and a recycled or AI-slop listicle usually carries nothing. This informs the number; it never sets a ceiling and never auto-skips a domain — a low-type page thick with verbatim quotes still scores on what it actually holds.

A page thick with usable material is useful even if its trust is low; a polished, sourceless page is near-empty even if its trust is high.

## 4. Write the judgment once

State both numbers with their one-line reasoning, then record them in a single call:

    python3 <profile>/scripts/sources.py judge <url> --trust <N> --useful <N> --note "<one-line reasoning>"

The domain and author averages update from this row automatically. If the command reports the URL is already judged, stop and report that — the judgment is final unless the Architect asks to overwrite.
