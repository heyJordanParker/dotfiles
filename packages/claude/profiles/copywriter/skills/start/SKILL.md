---
name: start
description: Scaffold one project folder inside a product's workspace — a page or a campaign, with its re-derived judged files, strategy folder, plan-file placeholders, and working layout. TRIGGER when a new page or campaign enters an existing workspace. DO NOT TRIGGER to initialize the whole product workspace (setup) or to fill in the strategy (plan-copy).
---

# Start a project

One Process: lay the folder one project works in. This is housekeeping done the moment work first lands, not a stage of the process. A project is one page or one campaign; a campaign holds its own pieces. The project's type lives in its folder name.

## 1. Place the folder

### Scaffold under projects/ with the type in the name
Create the project under `projects/`, naming it `<name>-page` for a standalone page, ad, email, post, or script, or `<name>-campaign` for a campaign. The suffix carries the type; name the rest plainly.

## 2. Lay the working layout

IF scaffolding a standalone page:
### Scaffold the re-derived judged files, plan files, and working folders
Inside the project folder, lay the project's own judged-file placeholders `Buyers.md`, `Competitors.md`, `Problems.md`, and `Statistics.md` (re-derived by plan-copy from the original research records, never copied from the root judged files), `strategies/` (the high-level strategy set and the owner's pick), the plan-file placeholders `Reader.md`, `Brief.md`, `Proof.md`, and `Wireframe.md`, plus `options/` (the option sets), `drafts/` (the copy), `findings/` (each check's per-round findings), and `rounds/` (the immutable round records). Scaffold `research/` (records only) only when this project commissions its own research; the commissioning `Brief.md` is written at the workspace root, never inside `research/`.

### Scaffold a launch sequence as one project
A launch or lifecycle email sequence is ONE project, not one per send: one folder, one `Reader.md` noting the awareness arc across the sends, and one draft file per email inside `drafts/`. Do not scaffold a folder per email.

IF scaffolding a campaign:
### Scaffold the campaign files and one folder per piece
Inside the campaign folder, lay `Campaign.md` (Goal · Offer with its named limitations · Timeline, one line per piece), the project's own re-derived `Buyers.md`, `Competitors.md`, `Problems.md`, and `Statistics.md`, the shared `Brief.md` and `Proof.md` the pieces read, `strategies/` for the campaign-level strategy set, and `findings/` and `rounds/` at the campaign level so the set-level review has somewhere to record and can terminate. Under it, scaffold one piece folder per deliverable the campaign carries; each piece folder carries its own `Reader.md`, `options/`, `drafts/`, `findings/`, and `rounds/`, and omits `Brief.md` and `Proof.md`, reading the campaign's. Scaffold a piece's `research/` (records only) only when that piece commissions its own research; the commissioning `Brief.md` is written at the workspace root, never inside `research/`.

## 3. Leave the strategy for plan-copy

### Lay placeholders only
start lays the folder and the placeholders. The re-derived judged files and the strategy are filled in later by plan-copy.

Verification: a standalone page placed under `projects/<name>-page/` with `Buyers.md`, `Competitors.md`, `Problems.md`, `Statistics.md`, `strategies/`, `Reader.md`, `Brief.md`, `Proof.md`, `Wireframe.md`, `options/`, `drafts/`, `findings/`, and `rounds/`; a launch sequence scaffolded as one folder with one `Reader.md` and one draft file per email in `drafts/`; a campaign placed under `projects/<name>-campaign/` with `Campaign.md`, the re-derived `Buyers.md`, `Competitors.md`, `Problems.md`, and `Statistics.md`, shared `Brief.md` and `Proof.md`, `strategies/`, `findings/`, and `rounds/`, and one piece folder per deliverable each carrying its own `Reader.md`, `options/`, `drafts/`, `findings/`, and `rounds/`; `research/` scaffolded records-only where research is commissioned, its commissioning `Brief.md` written at the workspace root and never inside `research/`; the judged files and strategy left for plan-copy.
