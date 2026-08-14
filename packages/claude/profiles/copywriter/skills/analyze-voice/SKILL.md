---
name: analyze-voice
description: Builds a Voice.md profile from a writer's existing samples — reverse-engineers the patterns that make their copy sound like them, then writes a structured, reusable profile the rest of the team applies at write time. TRIGGER when asked to capture, define, or learn a voice, build or regenerate a Voice.md, analyze writing samples for tone, or set up a new product's voice. DO NOT TRIGGER when writing or editing copy against an existing Voice.md — writing copy is the copywriting skill, matching it against the profile is the editor's job. This skill only produces the profile; it never writes the copy.
---

# Analyze Voice

A voice profile is what stops the team's copy reading like generic AI. This skill turns a writer's real samples into `Voice.md` — the file every writer and editor reads before they touch a line. Your job is to find the patterns that make the samples sound like one person, not to invent a voice.

### Follow setup's Voice.md contract
Fact-file contracts: setup's contract block governs every edit. Route any copywriting instruction ("write for one person", "use words the market already uses") or process context out of Voice.md — putting them in is the failure this skill most often makes.

## 1. Decide the tier by sample count

The honest output depends on how much real writing you have. Pick the tier, then build to its confidence ceiling — never claim more.

- **0 samples → aspirational.** No writing to analyze. Run a guided questionnaire (below) and write a target voice from the answers. Mark every section `aspirational` in the metadata and add the line: *unproven — written from intent, not samples. Regenerate from real writing before trusting it.*
- **1–4 samples → provisional.** Enough to sketch, not to confirm. Capture what the samples show, mark thin sections `provisional`, and note the profile firms up at 5+ samples.
- **5+ samples → full.** Reverse-engineer across all samples. Keep only patterns that recur in most of them; drop one-off quirks. This is the only tier that earns full confidence on its core sections.

## 2. Capture by reverse-engineering

For each sample, ask: *what would I have to tell a writer to produce exactly this?* Every answer is a captured rule — a word choice, a sentence shape, a punctuation habit, a thing the writer refuses to do.

- **Across 5+ samples**, keep the rules that recur in most samples and discard the rest. Recurrence is the test — a pattern in one sample is a coincidence, a pattern in four is the voice.
- **Pull signature phrases verbatim.** Exact recurring openers, transitions, and sign-offs are the highest-fidelity part of a voice. Copy them word for word; never paraphrase.
- **Separate the structural from the personal.** Product-voice patterns (register, banned words, sentence length, how it handles a feature) reproduce reliably. Subtle personal rhythm does not — see the ceiling below.

## 3. Capture with no samples (questionnaire)

When there's nothing to analyze, ask the writer — one question at a time:

- Name three writers, brands, or pages whose voice you'd want to sound like, and one you'd never want to sound like.
- What does your product never say? (slang, hype words, jargon, emoji, exclamation marks)
- Formal or casual? Do you address the reader as "you"?
- Write one sentence describing your product the way you'd actually say it out loud.

Build the aspirational profile from the answers. It is a target to test real writing against, not a description of writing that exists.

## 4. Validate before you ship the profile

Run these checks on the drafted `Voice.md` — fix what fails:

- **is/is-not presence** — every entry in the identity core pairs a concrete "is" with a concrete "is-not". A lone "is" ("clear and friendly") describes nothing; the contrast ("friendly, not chummy — warm without slang") is what's usable.
- **terminology presence** — the use/avoid term lists are non-empty and drawn from the samples, not generic. Generic banned-word lists belong in check-ai-writing; this section captures *this* writer's specific words.
- **reproduction test** — write one line in the captured voice and read it against the samples. If it doesn't pass as the same writer, the fingerprint is too thin — capture more, or drop the tier.

## 5. The honest ceiling

Structured product voice reproduces well; a subtle personal voice does not. Say so in the profile's metadata. Capturing register, vocabulary, and refusals gets the team most of the way; the last increment of a distinctive human cadence won't survive the round-trip. Over-claiming fidelity is the failure mode this caveat exists to prevent — a writer who trusts an over-confident profile ships copy that sounds almost-right, which is worse than obviously-generic.

## 6. Write the profile

Write `Voice.md` into the project's working directory — the project the copy is for, never inside this profile or its repo (that holds instructions, not work products). Normal case — never `VOICE.md`. Follow the section-by-section schema in [voice-schema.md](references/voice-schema.md); the schema is the single owner of the file's structure, so build every section it lists, at the confidence the tier allows.
