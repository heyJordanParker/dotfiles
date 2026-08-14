---
name: browse
description: Read one web page for research — a plain-HTTP fetch that passes bot walls headless browsers cannot, cleaned to its main content, cached, and recorded in the source registry. TRIGGER when a research or review step needs a page's text. DO NOT TRIGGER to screenshot a page or drive an interactive surface (screenshot, agent-browser), or to score a source you have already read (review-source).
---

# Browse

One Process — read one page through the registry gate. `browse.py` OWNS the reading: a browser-User-Agent plain-HTTP fetch (which passes bot walls that block headless browsers — 13/13 review quotes recovered on a Cloudflare-gated page where agent-browser and Playwright both failed), a deterministic readability cleaner, caching, and the one agent-browser fallback with its session closed inside the command. It records every outcome into the source registry, so an unreadable page is logged, never guessed.

## 1. Check the registry before reading

Check the domain's trust before you open a page:

    python3 <profile>/scripts/sources.py check <url>

Deprioritize a low-trust domain by reading it more skeptically — never auto-skip it. The check informs how you read, not whether you read.

## 2. Run browse.py and read the cached text it prints

    python3 <profile>/skills/browse/scripts/browse.py <url>

Exit 0 prints the path to the cleaned main-content text — read that file. A `<hash>.raw.txt` sits beside it with the full stripped text; pass `--raw` to print that path instead, and read it when the cleaner dropped something you need. A nonzero exit is the stop: the page could not be read and the registry already recorded the outcome (2 blocked, 3 gone/network, 4 invalid URL, 6 dead URL), so move on — never guess the page's contents.

Verification: the domain trust was checked; browse.py ran; on exit 0 you read the cached text it printed; on a nonzero exit you moved on without guessing, the outcome already recorded.
