---
name: publish
description: Turns cleared copy into a dated deliverable file in the project's working directory, with a header recording the asset, date, and brief. TRIGGER when copy has cleared every editor's checks and is ready to ship, hand off, save, or export as a final draft. DO NOT TRIGGER while copy is still being written or revised — writing is the copywriting skill, clearing it is the editing skill. Publish is the last step, only for copy that is already cleared.
---

# Publish

The last step. A draft becomes a deliverable when it leaves the workspace as a real, dated artifact someone can ship. This skill takes cleared copy and writes it to its destination.

## Before you publish

Publish only cleared copy — copy where every editor's checks pass (line, voice, slop, strategy) or the copy-chief has overridden a check on the record. Publishing a draft that hasn't cleared defeats the point of the pipeline. If the copy hasn't cleared, stop and hand it back to the editors; don't publish it.

## The destination

Publish does one thing: send cleared copy to its destination. Today the only destination is a file in the project's working directory — no content or email MCP server is connected yet. Keep two steps separate so that stays true when one is added:

1. **Assemble the deliverable** — the header plus the copy. Destination-independent.
2. **Write it to the destination** — today, a file in the working directory. When a content/email MCP joins the profile, only this step changes: it calls the MCP instead of writing a file. Step 1 and the precondition check above do not move.

Don't fold the file-writing into the assembly. The split is what lets a live destination slot in behind the same skill later.

## File shape

Write into the current working directory — the project the copy is for. Never write inside this profile or its repo: that holds instructions, not work products.

- **Name:** `YYYY-MM-DD-<slug>.md` — the date, then a short hyphenated slug from the asset (`2026-06-26-pricing-page-headline.md`). The date sorts the files chronologically; the slug says what it is at a glance.
- **Header:** a short block at the top of the file recording what shipped:

```
---
asset: Pricing page — hero headline + subhead
date: 2026-06-26
brief: One-line summary of who it's for and the action it drives.
status: cleared
---
```

- **Body:** the cleared copy, verbatim. Nothing added, nothing reformatted — the deliverable is the copy as it ships. Where the copy has parts (headline, subhead, call-to-action), label them so whoever places it knows what goes where.

## Steps

1. Confirm the copy is cleared (above).
2. Assemble the header from the asset, today's date, and the brief.
3. Slug the asset and write `YYYY-MM-DD-<slug>.md` into the working directory, header then copy.
4. Report the path written.
