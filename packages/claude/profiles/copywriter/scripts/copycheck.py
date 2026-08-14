#!/usr/bin/env python3
"""Deterministic copy checks for draft markdown.

Runs the mechanical passes that need no LLM: per-block counts against a platform
limit, per-paragraph reading level, sentence rhythm and paragraph length,
passive-voice density, contrastive and parallel-clause patterns, a banned-phrase
sweep from banned-phrases.txt, placeholder detection, and repeated-phrase
detection across sections. Plus copy-language flags (flag, never rewrite): em
dash characters, "and"-compound claims on headline/positioning lines, more than
one claim in a positioning line, and hyphenated adjective-and-adjective persona
labels ("burned-and-resigned"). Prints plain `file:line: issue` findings.

By default every finding is advisory and the exit code is 0. With --strict,
the four copy-language findings (em dash, "and"-compound claim, multi-claim
positioning, persona label) are BLOCKING: any of them makes the exit code 1,
and a missing input file also makes the exit code 1. Markdown heading lines
(#/##/###) and bold-only lines (**...** and nothing else) are always checked
as claim lines, so violations there block under --strict with no label needed.
All other findings stay advisory in both modes. Unlabeled BODY lines are the
remaining blind spot: their suspects stay warnings, and the WARNING count is
printed (strict included) so silence is not read as clean.

Stdlib only.

Usage:
    copycheck.py <file.md> [<file.md> ...] [--platform NAME] [--strict] [--list-platforms]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


# Per-block hard limits, in characters, for surfaces that actually have one.
# A landing-page draft has no platform limit, so counts fire only when the
# caller names a platform. Numbers are the public post limits.
PLATFORMS: dict[str, int] = {
    "tweet": 280,
    "x": 280,
    "sms": 160,
    "meta-headline": 40,       # Meta ad primary headline
    "meta-primary": 125,       # Meta ad primary text (above the fold)
    "google-headline": 30,     # Google RSA headline
    "google-description": 90,  # Google RSA description
    "linkedin-intro": 210,     # LinkedIn post before "see more"
    "email-subject": 60,       # inbox truncation guide
    "meta-description": 160,   # SEO meta description
    "title-tag": 60,           # SEO <title>
}

# Sentence and paragraph thresholds. Above these a human reader stalls; these
# are set high enough that ordinary marketing sentences pass and only genuine
# run-ons and text walls fire.
LONG_SENTENCE_WORDS = 40
WALL_SENTENCE_COUNT = 6
WALL_WORD_COUNT = 90
READING_GRADE_LIMIT = 8.0
READING_MIN_WORDS = 8
MONOTONE_SENTENCE_COUNT = 3
MONOTONE_WORD_SPREAD = 2
RHYTHM_WALL_SENTENCE_COUNT = 5
RHYTHM_WALL_WORDS = 25
PARAGRAPH_SENTENCE_LIMIT = 4
PASSIVE_DENSITY_LIMIT = 0.4
PASSIVE_MIN_SENTENCES = 2

# Repeated-phrase detection. Deliberate motifs in good copy are short (3-4
# words: "checks against Stripe"), so the window is set to 6 words to skip them.
# A headline echoed once in the body is standard craft, so the bar is 3+
# occurrences across 2+ sections — that is over-repetition, not reinforcement.
REPEAT_PHRASE_WORDS = 6
REPEAT_MIN_OCCURRENCES = 3

# Placeholder markers. Fixed strings a draft leaves behind; a whole-word `\b`
# guard keeps `TK`/`XXX` from matching inside real words.
PLACEHOLDER_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("TBD", re.compile(r"\bTBD\b", re.IGNORECASE)),
    ("TODO", re.compile(r"\bTODO\b", re.IGNORECASE)),
    ("FIXME", re.compile(r"\bFIXME\b", re.IGNORECASE)),
    ("TK placeholder", re.compile(r"\bTK\b")),
    ("XXX", re.compile(r"\bXXX\b")),
    ("lorem ipsum", re.compile(r"\blorem\b|\bipsum\b", re.IGNORECASE)),
    ("[link ...] placeholder", re.compile(r"\[link\b", re.IGNORECASE)),
    ("[CTA ...] placeholder", re.compile(r"\[cta\b", re.IGNORECASE)),
    ("[placeholder]", re.compile(r"\[placeholder", re.IGNORECASE)),
    ("'unsettled' marker", re.compile(r"\bunsettled\b", re.IGNORECASE)),
    ("'destination TBD' marker", re.compile(r"destination tbd", re.IGNORECASE)),
]

# Lines that carry markup or stage directions, not prose. These are stripped
# before the prose passes (sentences, banned phrases, repetition) but kept for
# the placeholder pass, which is meant to see the brackets.
# Copy-language flags. All are flag-only — the script never rewrites.
# Em dashes are banned in copy outright; en dashes framed by spaces are the
# same tell typed with the wrong key, so both fire.
EM_DASH = re.compile(r"—|\s–\s")
# A headline or positioning line is one whose bold label or section title
# matches this. Heuristic: it only sees lines the draft labels as such.
CLAIM_LABEL = re.compile(
    r"\b(h1|h2|headline|positioning|position|tagline|subhead|hero|title)\b",
    re.IGNORECASE,
)
POSITIONING_LABEL = re.compile(r"\bposition(ing)?\b", re.IGNORECASE)
# "and"-compound: an "and"/"&" whose both sides carry 2+ words, so plain
# noun pairs like "terms and conditions" (1 word each side) pass.
AND_COMPOUND = re.compile(r"(\S+\s+\S+)\s+(?:and|&)\s+(\S+\s+\S+)", re.IGNORECASE)
# Multi-claim separators inside one positioning line.
CLAIM_SEPARATOR = re.compile(r"—|\s–\s|;|\.\s|\s+vs\.?\s+", re.IGNORECASE)
# Hyphenated adjective-and-adjective persona labels: "burned-and-resigned".
PERSONA_HYPHEN = re.compile(r"\b[A-Za-z]+-(?:and|but|yet)-[A-Za-z]+\b")

HEADING = re.compile(r"^\s{0,3}#{1,6}\s")
LIST_MARKER = re.compile(r"^\s*(?:[-*+]|\d+\.)\s+")
HTML_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)
STAGE_DIRECTION = re.compile(r"\*\(.*?\)\*")      # *(design — ...)*
BRACKET_NOTE = re.compile(r"\[[^\]]*\]")          # [link — destination TBD]
BOLD_LABEL = re.compile(r"^\s*\*\*[^*]+\*\*\s*")  # **H1**, **Body**, **Title**
BOLD_ONLY = re.compile(r"^\s*\*\*([^*]+)\*\*\s*$")  # a line that is ONLY bold text
INLINE_MARKUP = re.compile(r"[*_`#>]")
SENTENCE_SPLIT = re.compile(r"(?<=[.!?])\s+(?=[A-Z0-9\"'“])")
WORD = re.compile(r"[A-Za-z0-9']+")
VOWEL_GROUP = re.compile(r"[aeiouy]+", re.IGNORECASE)
PASSIVE_VOICE = re.compile(
    r"\b(?:am|is|are|was|were|be|been|being|get|gets|got)\s+"
    r"(?:[A-Za-z]+ly\s+)?"
    r"(?:[A-Za-z]+ed|known|made|built|written|given|shown|seen|taken|driven|"
    r"found|held|told|sent|kept|left|paid|sold|read|set|cut|put)\b",
    re.IGNORECASE,
)
NOT_BUT = re.compile(r"\bnot\b[^.!?;]{1,80}\bbut\b", re.IGNORECASE)
CLAUSE_SPLIT = re.compile(r"\s*(?:,|;|:|\band\b)\s*", re.IGNORECASE)


def load_banned(path: Path) -> list[tuple[str, re.Pattern[str]]]:
    """Compile each non-comment line of the word list into a boundary-guarded
    case-insensitive pattern. `\b` on alphanumeric edges only, so hyphenated
    entries like `cutting-edge` still match cleanly."""
    entries: list[tuple[str, re.Pattern[str]]] = []
    if not path.exists():
        return entries
    for raw in path.read_text(encoding="utf-8").splitlines():
        phrase = raw.strip()
        if not phrase or phrase.startswith("#"):
            continue
        pattern = re.compile(r"(?<!\w)" + re.escape(phrase) + r"(?!\w)", re.IGNORECASE)
        entries.append((phrase, pattern))
    return entries


def strip_markup(line: str) -> str:
    """Reduce a source line to its prose. Drops HTML comments, bracket notes,
    italic stage directions, a leading bold label, list markers, then inline
    markup characters. Returns '' for a line that was pure markup."""
    text = HTML_COMMENT.sub(" ", line)
    text = STAGE_DIRECTION.sub(" ", text)
    text = BRACKET_NOTE.sub(" ", text)
    text = BOLD_LABEL.sub("", text)
    text = LIST_MARKER.sub("", text)
    if HEADING.match(text):
        return ""
    text = INLINE_MARKUP.sub("", text)
    return re.sub(r"\s+", " ", text).strip()


def count_words(text: str) -> int:
    return len(WORD.findall(text))


def iter_prose_lines(lines: list[str]):
    """Yield (line_number, prose) for lines that carry real copy, tracking the
    active section title so cross-section repetition can name where a phrase
    landed. Skips the title-chain block, whose job is to duplicate the section
    titles verbatim and would otherwise flood the repetition pass."""
    section = "(top)"
    in_title_chain = False
    for number, line in enumerate(lines, start=1):
        stripped = line.strip()
        heading = HEADING.match(line)
        if heading:
            title = re.sub(r"^\s{0,3}#{1,6}\s*", "", line).strip()
            section = re.sub(r"\*", "", title)
            in_title_chain = bool(re.search(r"title chain|skim story", title, re.IGNORECASE))
            continue
        if in_title_chain:
            continue
        prose = strip_markup(line)
        if prose:
            yield number, section, prose


def iter_prose_paragraphs(lines: list[str]):
    """Yield blank-line-separated prose paragraphs with their first source line.
    Headings and the generated title-chain block remain outside body copy.
    List markers and fenced code blocks start their own units."""
    section = "(top)"
    in_title_chain = False
    paragraph_lines: list[tuple[int, str]] = []
    for number, line in enumerate(lines, start=1):
        heading = HEADING.match(line)
        if heading:
            if paragraph_lines:
                yield (
                    paragraph_lines[0][0],
                    section,
                    " ".join(prose for _, prose in paragraph_lines),
                )
                paragraph_lines = []
            title = re.sub(r"^\s{0,3}#{1,6}\s*", "", line).strip()
            section = re.sub(r"\*", "", title)
            in_title_chain = bool(re.search(r"title chain|skim story", title, re.IGNORECASE))
            continue
        if in_title_chain:
            continue
        if re.match(r"^\s*(?:```|~~~)", line):
            if paragraph_lines:
                yield (
                    paragraph_lines[0][0],
                    section,
                    " ".join(text for _, text in paragraph_lines),
                )
                paragraph_lines = []
            continue
        prose = strip_markup(line)
        if prose:
            if LIST_MARKER.match(line) and paragraph_lines:
                yield (
                    paragraph_lines[0][0],
                    section,
                    " ".join(text for _, text in paragraph_lines),
                )
                paragraph_lines = []
            paragraph_lines.append((number, prose))
        elif paragraph_lines:
            yield (
                paragraph_lines[0][0],
                section,
                " ".join(text for _, text in paragraph_lines),
            )
            paragraph_lines = []
    if paragraph_lines:
        yield (
            paragraph_lines[0][0],
            section,
            " ".join(text for _, text in paragraph_lines),
        )


def check_platform(lines: list[str], limit: int, findings: list, name: str) -> None:
    """Flag any prose block whose character count exceeds the named platform
    limit. Blocks are blank-line separated; the finding cites the first line."""
    block_lines: list[tuple[int, str]] = []

    def flush() -> None:
        if not block_lines:
            return
        text = " ".join(p for _, p in block_lines)
        chars = len(text)
        if chars > limit:
            start = block_lines[0][0]
            findings.append(
                (start,
                 f"block is {chars} chars, over the {name} limit of {limit} "
                 f"({chars - limit} over)",
                 False)
            )

    for number, line in enumerate(lines, start=1):
        if HEADING.match(line):
            flush()
            block_lines = []
            continue
        prose = strip_markup(line)
        if prose:
            block_lines.append((number, prose))
        else:
            flush()
            block_lines = []
    flush()


def check_sentences(prose_lines, findings: list) -> dict[str, float]:
    """Flag over-long sentences and text walls; return corpus sentence stats."""
    sentence_lengths: list[int] = []
    for number, _section, prose in prose_lines:
        sentences = [s for s in SENTENCE_SPLIT.split(prose) if s.strip()]
        words_here = count_words(prose)
        for sentence in sentences:
            length = count_words(sentence)
            if length:
                sentence_lengths.append(length)
            if length > LONG_SENTENCE_WORDS:
                findings.append(
                    (number,
                     f"long sentence, {length} words "
                     f"(over {LONG_SENTENCE_WORDS}): \"{_clip(sentence)}\"",
                      False)
                )
        if len(sentences) >= WALL_SENTENCE_COUNT or words_here >= WALL_WORD_COUNT:
            findings.append(
                (number,
                 f"wall of text, {words_here} words in {len(sentences)} sentences",
                 False)
            )

    if not sentence_lengths:
        return {"count": 0, "mean": 0.0, "max": 0}
    return {
        "count": len(sentence_lengths),
        "mean": round(sum(sentence_lengths) / len(sentence_lengths), 1),
        "max": max(sentence_lengths),
    }


def check_paragraph_quality(paragraphs, findings: list) -> None:
    """Flag deterministic rhythm and paragraph-length patterns."""
    for number, _section, prose in paragraphs:
        sentences = [s for s in SENTENCE_SPLIT.split(prose) if s.strip()]
        lengths_here = [count_words(sentence) for sentence in sentences]
        long_run = 0
        longest_long_run = 0
        for length in lengths_here:
            long_run = long_run + 1 if length > RHYTHM_WALL_WORDS else 0
            longest_long_run = max(longest_long_run, long_run)
        if longest_long_run >= RHYTHM_WALL_SENTENCE_COUNT:
            findings.append(
                (number,
                 f"sentence-rhythm wall, {longest_long_run} consecutive sentences "
                 f"all over {RHYTHM_WALL_WORDS} words (mechanical length test; "
                 f"cannot judge whether complexity is necessary)",
                 False)
            )
        if len(sentences) > PARAGRAPH_SENTENCE_LIMIT:
            findings.append(
                (number,
                 f"long body paragraph, {len(sentences)} sentences "
                 f"(over {PARAGRAPH_SENTENCE_LIMIT}; heuristic assumes blank-line "
                 f"separation marks paragraphs)",
                 False)
            )

        monotone_windows = [
            index
            for index in range(len(lengths_here) - MONOTONE_SENTENCE_COUNT + 1)
            if max(lengths_here[index : index + MONOTONE_SENTENCE_COUNT])
            - min(lengths_here[index : index + MONOTONE_SENTENCE_COUNT])
            <= MONOTONE_WORD_SPREAD
        ]
        if monotone_windows:
            start = monotone_windows[0]
            end = start + MONOTONE_SENTENCE_COUNT
            for index in monotone_windows[1:]:
                if index <= end - MONOTONE_SENTENCE_COUNT + 1:
                    end = index + MONOTONE_SENTENCE_COUNT
                else:
                    run = lengths_here[start:end]
                    findings.append(
                        (number,
                         f"monotone sentence run, {len(run)} consecutive lengths "
                         f"{run} (spread at most {MONOTONE_WORD_SPREAD} words; "
                         f"punctuation-based sentence split can misread abbreviations)",
                         False)
                    )
                    start = index
                    end = index + MONOTONE_SENTENCE_COUNT
            run = lengths_here[start:end]
            findings.append(
                (number,
                 f"monotone sentence run, {len(run)} consecutive lengths {run} "
                 f"(spread at most {MONOTONE_WORD_SPREAD} words; punctuation-based "
                 f"sentence split can misread abbreviations)",
                 False)
            )


def _count_syllables(word: str) -> int:
    """Estimate English syllables from vowel groups; deliberately conservative."""
    normalized = re.sub(r"[^a-z]", "", word.lower())
    if not normalized:
        return 0
    syllables = len(VOWEL_GROUP.findall(normalized))
    if normalized.endswith("e") and not normalized.endswith(("le", "ye")) and syllables > 1:
        syllables -= 1
    return max(1, syllables)


def check_reading_level(paragraphs, findings: list, reports: list) -> dict[str, float]:
    """Report Flesch-Kincaid grade per paragraph and flag high body-copy grades."""
    grades: list[float] = []
    for number, _section, prose in paragraphs:
        sentences = [s for s in SENTENCE_SPLIT.split(prose) if s.strip()]
        words = WORD.findall(prose)
        if not sentences or not words:
            continue
        syllables = sum(_count_syllables(word) for word in words)
        grade = round(
            0.39 * (len(words) / len(sentences))
            + 11.8 * (syllables / len(words))
            - 15.59,
            1,
        )
        grades.append(grade)
        reports.append(
            (number,
             f"reading level grade {grade:.1f} "
             f"({len(words)} words/{len(sentences)} sentences; Flesch-Kincaid "
             f"with a vowel-group syllable estimate)")
        )
        if len(words) >= READING_MIN_WORDS and grade > READING_GRADE_LIMIT:
            findings.append(
                (number,
                 f"high body-copy reading level, grade {grade:.1f} "
                 f"(over {READING_GRADE_LIMIT:.0f}; heuristic syllable counts can "
                 f"skew on names, acronyms, and non-English words)",
                 False)
            )
    return {
        "count": len(grades),
        "mean": round(sum(grades) / len(grades), 1) if grades else 0.0,
        "max": max(grades) if grades else 0.0,
    }


def check_mechanical_style(paragraphs, findings: list) -> None:
    """Flag measurable style patterns without claiming they are always wrong."""
    for number, _section, prose in paragraphs:
        sentences = [s for s in SENTENCE_SPLIT.split(prose) if s.strip()]
        passive_sentences = sum(1 for sentence in sentences if PASSIVE_VOICE.search(sentence))
        if (
            len(sentences) >= PASSIVE_MIN_SENTENCES
            and passive_sentences / len(sentences) >= PASSIVE_DENSITY_LIMIT
        ):
            density = round(100 * passive_sentences / len(sentences))
            findings.append(
                (number,
                 f"possible passive-voice density {density}% "
                 f"({passive_sentences}/{len(sentences)} sentences; heuristic "
                 f"matches be/get + common participles, so adjectival uses can "
                 f"false-positive and uncommon irregular verbs can be missed)",
                 False)
            )

        contrasts = len(NOT_BUT.findall(prose))
        if contrasts:
            findings.append(
                (number,
                 f"\"not X but Y\" contrastive pattern {contrasts}x "
                 f"(mechanical not…but match within 80 characters; cannot judge "
                 f"whether the contrast is useful)",
                 False)
            )

        for sentence in sentences:
            clauses = [clause.strip() for clause in CLAUSE_SPLIT.split(sentence)]
            openings = [
                WORD.findall(clause)[0].lower()
                for clause in clauses
                if WORD.findall(clause)
            ]
            repeated = next(
                (opening for opening in openings if openings.count(opening) >= 3),
                None,
            )
            if repeated:
                findings.append(
                    (number,
                     f"triple parallel clauses repeat opening \"{repeated}\" "
                     f"{openings.count(repeated)}x (comma/semicolon/colon/and "
                     f"split; intentional rhetorical triples can match)",
                     False)
                )
                break


def check_banned(prose_lines, banned, findings: list) -> None:
    for number, _section, prose in prose_lines:
        for phrase, pattern in banned:
            if pattern.search(prose):
                findings.append((number, f"banned phrase \"{phrase}\"", False))


def check_placeholders(lines: list[str], findings: list) -> None:
    """Runs on RAW lines — placeholders live in the brackets that the prose
    passes strip out. One finding per line: a single `[link — destination TBD]`
    trips three patterns, so collapse them into one labelled list."""
    for number, line in enumerate(lines, start=1):
        text = HTML_COMMENT.sub(" ", line)
        hit_labels: list[str] = []
        for label, pattern in PLACEHOLDER_PATTERNS:
            if pattern.search(text):
                hit_labels.append(label)
        if hit_labels:
            findings.append((number, f"placeholder ({', '.join(hit_labels)})", False))


def check_repetition(prose_lines, findings: list) -> None:
    """Flag a phrase of REPEAT_PHRASE_WORDS words that appears
    REPEAT_MIN_OCCURRENCES times or more across two or more distinct sections.
    Two occurrences is ordinary reinforcement (a headline echoed in the body);
    three-plus is over-repetition. Overlapping windows of the same run collapse
    to the single longest phrase."""
    occurrences: dict[str, list[tuple[int, str]]] = {}
    for number, section, prose in prose_lines:
        tokens = [w.lower() for w in WORD.findall(prose)]
        for index in range(len(tokens) - REPEAT_PHRASE_WORDS + 1):
            gram = " ".join(tokens[index : index + REPEAT_PHRASE_WORDS])
            occurrences.setdefault(gram, []).append((number, section))

    qualified = {
        gram: hits
        for gram, hits in occurrences.items()
        if len(hits) >= REPEAT_MIN_OCCURRENCES
        and len({section for _, section in hits}) >= 2
    }
    # Drop a phrase wholly contained in another qualified phrase, so one long
    # duplicated run yields one finding, not one per sliding window.
    grams = sorted(qualified, key=len, reverse=True)
    kept = [g for i, g in enumerate(grams) if not any(g in longer for longer in grams[:i])]

    for gram in sorted(kept, key=lambda g: qualified[g][0][0]):
        hits = qualified[gram]
        lines_hit = ", ".join(str(line) for line, _ in hits)
        findings.append(
            (hits[0][0],
             f"phrase repeats {len(hits)}x across sections "
             f"(lines {lines_hit}): \"{gram}\"",
             False)
        )


def check_copy_language(lines: list[str], findings: list) -> int:
    """Mechanical copy-language flags. Flag, never rewrite. These four are the
    BLOCKING findings under --strict.

    Heuristic limits, stated so readers weigh the findings: markdown heading
    lines and bold-only lines are always checked as claim lines; other lines
    are recognized as headline/positioning only by a bold label or section
    title (h1/headline/positioning/...), so unlabeled BODY claim lines are not
    checked; the "and"-compound check flags any and/& with 2+ words on each
    side and cannot tell a compound claim from an ordinary compound phrase;
    the multi-claim count splits on dashes, semicolons, periods, and "vs" and
    cannot judge whether the parts are truly separate claims.

    Returns the count of UNLABELED suspect lines: body lines outside labeled
    headline/positioning regions carrying an em dash or an "and"-compound
    shape. The label-dependent checks never saw them, so the caller prints a
    WARNING count — silence must not be read as clean."""
    unlabeled_suspects = 0
    section = "(top)"
    for number, line in enumerate(lines, start=1):
        if HEADING.match(line):
            section = re.sub(r"^\s{0,3}#{1,6}\s*", "", line).strip()

        prose = strip_markup(line)
        heading = bool(HEADING.match(line))
        if heading:
            prose = re.sub(r"^\s{0,3}#{1,6}\s*", "", INLINE_MARKUP.sub("", line)).strip()
        bold_only = BOLD_ONLY.match(line)
        if not prose and bold_only:
            # strip_markup treats a bold-only line as a label and empties it;
            # its text IS the copy, so recover it and check it as a claim line.
            prose = INLINE_MARKUP.sub("", bold_only.group(1)).strip()
        if not prose:
            continue

        # Bracket notes and stage directions are markup, not copy; their
        # dashes do not fire.
        copy_text = BRACKET_NOTE.sub(" ", STAGE_DIRECTION.sub(" ", HTML_COMMENT.sub(" ", line)))
        if EM_DASH.search(copy_text):
            findings.append((number, "em dash (banned in copy)", True))

        if PERSONA_HYPHEN.search(prose):
            hit = PERSONA_HYPHEN.search(prose).group(0)
            findings.append(
                (number, f"hyphenated persona label \"{hit}\" (made-up compound)", True)
            )

        label_match = BOLD_LABEL.match(line)
        label = label_match.group(0) if label_match else ""
        is_claim_line = heading or bool(bold_only) or bool(
            CLAIM_LABEL.search(label) or CLAIM_LABEL.search(section)
        )
        is_positioning_line = bool(
            POSITIONING_LABEL.search(label) or POSITIONING_LABEL.search(section)
        )

        if is_claim_line or is_positioning_line:
            compound = AND_COMPOUND.search(prose)
            if compound:
                findings.append(
                    (number,
                     f"\"and\"-compound claim on headline/positioning line: "
                     f"\"{_clip(prose)}\" (heuristic: any and/& with 2+ words each "
                     f"side; cannot tell claims from compound phrases)",
                     True)
                )

        if not (is_claim_line or is_positioning_line):
            if EM_DASH.search(copy_text) or AND_COMPOUND.search(prose):
                unlabeled_suspects += 1

        if is_positioning_line:
            parts = [
                part for part in CLAIM_SEPARATOR.split(prose)
                if part and count_words(part) >= 3
            ]
            if len(parts) >= 2:
                findings.append(
                    (number,
                     f"positioning line may carry {len(parts)} claims "
                     f"(one claim per positioning): \"{_clip(prose)}\" (heuristic: "
                     f"split on dash/semicolon/period/vs; cannot judge claim-ness)",
                     True)
                )
    return unlabeled_suspects


def _clip(text: str, width: int = 60) -> str:
    text = text.strip()
    return text if len(text) <= width else text[: width - 1].rstrip() + "…"


def check_file(path: Path, banned, platform: str | None) -> int:
    """Print the file's findings; return the number of BLOCKING findings."""
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    prose_lines = list(iter_prose_lines(lines))
    paragraphs = list(iter_prose_paragraphs(lines))
    findings: list[tuple[int, str, bool]] = []
    reports: list[tuple[int, str]] = []

    if platform:
        check_platform(lines, PLATFORMS[platform], findings, platform)
    stats = check_sentences(paragraphs, findings)
    check_paragraph_quality(paragraphs, findings)
    reading_stats = check_reading_level(paragraphs, findings, reports)
    check_mechanical_style(paragraphs, findings)
    check_banned(prose_lines, banned, findings)
    check_placeholders(lines, findings)
    check_repetition(prose_lines, findings)
    unlabeled_suspects = check_copy_language(lines, findings)

    findings.sort(key=lambda f: f[0])
    name = str(path)
    output = [
        (line, f"METRIC: {message}", False)
        for line, message in reports
    ] + findings
    output.sort(key=lambda item: item[0])
    for line, message, blocking in output:
        marker = "BLOCKING: " if blocking else ""
        print(f"{name}:{line}: {marker}{message}")

    if unlabeled_suspects:
        print(
            f"{name}: WARNING: {unlabeled_suspects} unlabeled suspect "
            f"line{'s' if unlabeled_suspects != 1 else ''} (em dash or "
            f"and-compound shape outside labeled headline/positioning regions; "
            f"label-dependent checks did not judge them — silence is not clean)",
            file=sys.stderr,
        )

    total = len(findings)
    blocking_total = sum(1 for _, _, blocking in findings if blocking)
    print(
        f"{name}: {total} finding{'s' if total != 1 else ''} "
        f"({blocking_total} blocking) | "
        f"{stats['count']} sentences, mean {stats['mean']} words, "
        f"longest {stats['max']} | "
        f"{reading_stats['count']} paragraphs, mean grade "
        f"{reading_stats['mean']}, highest {reading_stats['max']}",
        file=sys.stderr,
    )
    return blocking_total


def main() -> int:
    argument_parser = argparse.ArgumentParser(
        description="Deterministic copy checks for draft markdown."
    )
    argument_parser.add_argument("files", nargs="*", help="markdown files to check")
    argument_parser.add_argument(
        "--platform",
        choices=sorted(PLATFORMS),
        help="enforce this surface's per-block character limit",
    )
    argument_parser.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 when any blocking finding fires (em dash, and-compound "
        "claim, multi-claim positioning, persona label) or an input file is missing",
    )
    argument_parser.add_argument(
        "--list-platforms",
        action="store_true",
        help="print the known platform limits and exit",
    )
    arguments = argument_parser.parse_args()

    if arguments.list_platforms:
        for name in sorted(PLATFORMS):
            print(f"{name}: {PLATFORMS[name]} chars")
        return 0

    if not arguments.files:
        argument_parser.error("give at least one file, or use --list-platforms")

    banned = load_banned(Path(__file__).with_name("banned-phrases.txt"))
    blocking_total = 0
    missing_total = 0
    for raw_path in arguments.files:
        path = Path(raw_path)
        if not path.exists():
            print(f"{raw_path}: file not found", file=sys.stderr)
            missing_total += 1
            continue
        blocking_total += check_file(path, banned, arguments.platform)
    if arguments.strict and (blocking_total or missing_total):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
