#!/usr/bin/env python3
"""Scan Claude transcripts for the Architect's own messages matching a pattern.

Prints each match with the assistant text that preceded it, newest sessions first.
Usage: scan_corrections.py <regex> [--days N] [--before-chars N] [--limit N]
"""
import argparse
import json
import re
import sys
import time
from pathlib import Path

ROOTS = [
    Path.home() / ".claude/projects/-Users-jordan-Developer-creator-income-blueprint-worktrees-design",
    Path.home() / ".claude/projects/-Users-jordan-Developer-creator-income-blueprint",
    Path.home() / ".claude/projects/-Users-jordan-dotfiles",
]


def text_of(record):
    message = record.get("message") or {}
    content = message.get("content")
    if isinstance(content, str):
        return content
    if not isinstance(content, list):
        return ""
    parts = []
    for block in content:
        if isinstance(block, dict) and block.get("type") == "text":
            parts.append(block.get("text") or "")
    return "\n".join(parts)


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
    return body


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("pattern")
    parser.add_argument("--days", type=int, default=45)
    parser.add_argument("--before-chars", type=int, default=900)
    parser.add_argument("--limit", type=int, default=25)
    args = parser.parse_args()

    needle = re.compile(args.pattern, re.IGNORECASE)
    cutoff = time.time() - args.days * 86400

    files = []
    for root in ROOTS:
        if root.is_dir():
            files.extend(p for p in root.glob("*.jsonl") if p.stat().st_mtime >= cutoff)
    files.sort(key=lambda p: p.stat().st_mtime, reverse=True)

    shown = 0
    for path in files:
        previous_assistant = ""
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
                body = text_of(record).strip()
                if body:
                    previous_assistant = body
                continue
            said = architect_said(record)
            if said is None:
                continue
            if needle.search(said):
                stamp = record.get("timestamp", "")[:16]
                print("=" * 78)
                print(f"{stamp}  {path.name}")
                print("--- agent before ---")
                print(previous_assistant[-args.before_chars:])
                print("--- architect ---")
                print(said[:1400])
                shown += 1
                if shown >= args.limit:
                    return 0
            previous_assistant = ""
    if not shown:
        print("no matches", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
