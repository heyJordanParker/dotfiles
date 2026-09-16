#!/usr/bin/env python3
"""Measure how option sets actually landed with the Architect.

For every assistant message carrying an option set, look at the Architect's next
message and classify it: did he pick a non-recommended option, accept, reject the
options themselves, or ignore them. Also counts the times he asked for options.
"""
import json
import re
import sys
from pathlib import Path

ROOTS = [
    Path.home() / ".claude/projects/-Users-jordan-Developer-creator-income-blueprint-worktrees-design",
    Path.home() / ".claude/projects/-Users-jordan-Developer-creator-income-blueprint",
    Path.home() / ".claude/projects/-Users-jordan-dotfiles",
    Path.home() / ".claude/projects/-Users-jordan-Developer-creator-income-blueprint-worktrees-production",
]

OPTION_SET = re.compile(r"(\*\*Option [0-9A-Z]|^Option [0-9A-Z][:.]|Confidence: ?\d{1,3}%)", re.M)
SECOND_OPTION = re.compile(r"(\*\*Option (?:2|3|4|B|C|D)|^Option (?:2|3|4|B|C|D)[:.])", re.M)
ASKED = re.compile(r"(give me (the )?options|what are the options|/pcc|propose the options|options to choose|pcc)", re.I)
PICKED = re.compile(r"\boption ?([0-9A-D])\b", re.I)
REJECTS_OPTIONS = re.compile(
    r"(bad option|dumb option|stupid option|useless option|padding|don't want (bad|those) "
    r"|rejected|why (the fuck )?would (i|we)|not making dumb|i'm not (making|choosing)"
    r"|stop (fucking )?using options|vomit|shit out (bad )?options|obviously bad)",
    re.I,
)
REJECTS_ANY = re.compile(r"(^no\b|no\.|wrong|terrible|shit|fuck|incoherent|dodgy|retarded|stupid|i don't (understand|want))", re.I)


def text_of(record):
    message = record.get("message") or {}
    content = message.get("content")
    if isinstance(content, str):
        return content
    if not isinstance(content, list):
        return ""
    return "\n".join(
        block.get("text") or ""
        for block in content
        if isinstance(block, dict) and block.get("type") == "text"
    )


def architect_said(record):
    if record.get("type") != "user":
        return None
    message = record.get("message") or {}
    if message.get("role") != "user":
        return None
    content = message.get("content")
    if isinstance(content, list):
        for block in content:
            if isinstance(block, dict) and block.get("type") == "tool_result":
                return None
    body = text_of(record).strip()
    if not body or body.startswith("<") or body.startswith("Caveat:"):
        return None
    if body.startswith("Base directory for this skill:"):
        return None
    if body.startswith("This session is being continued"):
        return None
    if body.startswith("Another Claude session sent a message:"):
        return None
    return body


def main():
    sets_shown = 0
    multi_option_sets = 0
    followed_by_rejection_of_options = 0
    followed_by_any_rejection = 0
    picked_non_first = 0
    asked_for_options = 0
    architect_turns = 0
    multi_after_request = 0
    samples = []
    picks = []

    for root in ROOTS:
        if not root.is_dir():
            continue
        for path in sorted(root.glob("*.jsonl")):
            pending = None
            last_ask = False
            try:
                lines = path.read_text(errors="replace").splitlines()
            except OSError:
                continue
            for line in lines:
                try:
                    record = json.loads(line)
                except ValueError:
                    continue
                if record.get("type") == "assistant":
                    body = text_of(record)
                    if body.strip() and OPTION_SET.search(body):
                        pending = (body, record.get("timestamp", "")[:10], path.name, last_ask)
                    elif body.strip():
                        pending = None
                    continue
                said = architect_said(record)
                if said is None:
                    continue
                architect_turns += 1
                last_ask = bool(ASKED.search(said))
                if last_ask:
                    asked_for_options += 1
                if pending:
                    body, stamp, name, was_asked = pending
                    sets_shown += 1
                    multi = bool(SECOND_OPTION.search(body))
                    if multi:
                        multi_option_sets += 1
                        if was_asked:
                            multi_after_request += 1
                    hit = PICKED.search(said)
                    if hit and hit.group(1).lower() not in ("1", "a"):
                        picked_non_first += 1
                        if len(picks) < 10:
                            picks.append((stamp, name, said[:240]))
                    if REJECTS_OPTIONS.search(said):
                        followed_by_rejection_of_options += 1
                        if len(samples) < 12:
                            samples.append((stamp, name, said[:320]))
                    elif REJECTS_ANY.search(said):
                        followed_by_any_rejection += 1
                    pending = None

    print(f"architect turns scanned                : {architect_turns}")
    print(f"agent replies carrying an option set   : {sets_shown}")
    print(f"  of those, two or more options        : {multi_option_sets}")
    print(f"    volunteered, he had not asked      : {multi_option_sets - multi_after_request}")
    print(f"    given after he asked for options   : {multi_after_request}")
    print(f"  he rejected the OPTIONS themselves   : {followed_by_rejection_of_options}")
    print(f"  he rejected the reply some other way : {followed_by_any_rejection}")
    print(f"  he picked an option other than #1    : {picked_non_first}")
    print(f"architect turns asking for options     : {asked_for_options}")
    print()
    print("--- samples of option rejections ---")
    for stamp, name, said in samples:
        print(f"{stamp}  {name}")
        print(f"  {said}")
        print()
    print("--- samples where he named a non-first option ---")
    for stamp, name, said in picks:
        print(f"{stamp}  {name}")
        print(f"  {said}")
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
