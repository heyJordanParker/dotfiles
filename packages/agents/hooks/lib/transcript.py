"""Shared Claude transcript layer.

Locates, reads, and parses Claude Code's JSONL transcript and exposes the views
hooks compose from — records, message blocks, the current turn, the conversation
stream, plan content, recent user messages, tool outcomes — so each hook stops
hand-rolling its own line-by-line parse and scattering `[:N]` truncations.

Bounding lives here as `clamp`, but it is the caller's choice: a Stop-event hook
can afford a generous ceiling, the every-prompt classifier keeps its lines tight.
Parsing is shared; the bound is not.
"""

import json
import os

# Generous default bound — a pathological multi-MB-turn guard, not a content cap.
CEILING = 200000
EDIT_TOOLS = ("Edit", "Write", "MultiEdit", "NotebookEdit")


def clamp(text, limit=CEILING):
    if not limit or len(text) <= limit:
        return text
    return text[:limit]


def records(path):
    """Parsed JSONL records in file (chronological) order; bad lines skipped."""
    if not path or not os.path.isfile(path):
        return []
    out = []
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    out.append(json.loads(line))
                except Exception:
                    continue
    except Exception:
        return []
    return out


def _content(record):
    msg = record.get("message")
    return msg.get("content") if isinstance(msg, dict) else None


def blocks(record, kind=None):
    """Dict content-blocks of a record, optionally filtered to one block type."""
    c = _content(record)
    if not isinstance(c, list):
        return []
    if kind is None:
        return [b for b in c if isinstance(b, dict)]
    return [b for b in c if isinstance(b, dict) and b.get("type") == kind]


def is_real_user(record):
    """A genuine user turn, not a tool-result delivery.

    The boundary is `"type":"user"` && no `tool_use_id`: a user record whose
    content is a plain string, or a list carrying no tool_result block."""
    if record.get("type") != "user":
        return False
    c = _content(record)
    if isinstance(c, list):
        return not any(isinstance(b, dict) and b.get("type") == "tool_result" for b in c)
    return True


def current_turn(recs):
    """Records after the last genuine user message, chronological."""
    cut = 0
    for i, r in enumerate(recs):
        if is_real_user(r):
            cut = i + 1
    return recs[cut:]


def current_turn_lines(path):
    """Raw JSONL lines of the current turn, chronological.

    Two completion-validator gates scan the turn's raw line text for substrings
    (`"name":"Edit"`, `ExitPlanMode`) — deliberately looser than parsed semantics,
    so they need the original bytes, not re-serialized
    records. Boundary matches current_turn: walk from the end, stop (exclusive) at
    the first genuine user line (`"type":"user"` without `tool_use_id`)."""
    if not path or not os.path.isfile(path):
        return []
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            lines = fh.read().split("\n")
    except Exception:
        return []
    collected = []
    for line in reversed(lines):
        if '"type":"user"' in line and "tool_use_id" not in line:
            break
        collected.append(line)
    collected.reverse()
    return collected


def tool_outcomes(recs):
    """tool_use_id -> failed(bool), read from tool_result blocks (is_error true)."""
    out = {}
    for r in recs:
        for b in blocks(r, "tool_result"):
            tid = b.get("tool_use_id")
            if tid:
                out[tid] = bool(b.get("is_error"))
    return out


def _tool_target(block):
    name = block.get("name", "")
    inp = block.get("input") or {}
    if name in EDIT_TOOLS:
        return inp.get("file_path") or inp.get("notebook_path") or ""
    if name == "Bash":
        cmd = inp.get("command", "")
        return (cmd[:120] + "…") if len(cmd) > 120 else cmd
    return ""


def turn_evidence(turn_recs):
    """Dense, accurate evidence for a completion check, chronological, unbounded.

    Every assistant response's text in full (the burial surface — a deliverable
    stranded in an earlier response), thinking as a size marker (no consumer needs
    its content), and each tool call as one line carrying its real outcome — paired
    to its tool_result by id, `ok` or `FAILED` — plus an edit tally, so a caller is
    never told work finished when an edit failed. Raw tool content is dropped."""
    outcomes = tool_outcomes(turn_recs)
    stream = []
    ok = failed = 0
    for r in turn_recs:
        if r.get("type") != "assistant":
            continue
        for b in blocks(r):
            t = b.get("type")
            if t == "text":
                stream.append("[response] %s" % b.get("text", ""))
            elif t == "thinking":
                stream.append("[thinking: %d chars]" % len(b.get("thinking", "")))
            elif t == "tool_use":
                name = b.get("name", "")
                bad = outcomes.get(b.get("id"))
                res = "FAILED" if bad else "ok"
                tgt = _tool_target(b)
                stream.append(("[%s %s -> %s]" % (name, tgt, res)) if tgt
                              else ("[%s -> %s]" % (name, res)))
                if name in EDIT_TOOLS:
                    if bad:
                        failed += 1
                    else:
                        ok += 1
    if ok or failed:
        stream.append("Edits this turn: %d finished, %d failed." % (ok, failed))
    return "\n".join(stream)


def assistant_text_len(recs):
    """Total assistant text length, +1 per block (a jq -r per-value newline), for
    the deliverable-shaped-turn threshold."""
    total = 0
    for r in recs:
        if r.get("type") != "assistant":
            continue
        for b in blocks(r, "text"):
            total += len(b.get("text", "")) + 1
    return total


def plan_content(recs):
    """The last ExitPlanMode plan in the transcript, else the slug's plan file on
    disk. Unbounded — the caller clamps."""
    plans = []
    slug = ""
    for r in recs:
        if not slug and isinstance(r.get("slug"), str):
            slug = r["slug"]
        if r.get("type") != "assistant":
            continue
        for b in blocks(r, "tool_use"):
            if b.get("name") == "ExitPlanMode":
                plan = (b.get("input") or {}).get("plan")
                if isinstance(plan, str):
                    plans.append(plan)
    if plans:
        return plans[-1]
    if slug:
        path = os.path.join(os.path.expanduser("~"), ".claude", "plans", "%s.md" % slug)
        if os.path.isfile(path):
            try:
                with open(path, encoding="utf-8", errors="replace") as fh:
                    return fh.read()
            except Exception:
                return ""
    return ""


def recent_user_texts(recs, n=4):
    """Text of the last n user messages, joined. Unbounded — the caller clamps.

    A user record contributes its scalar string content, or the joined text of its
    text blocks; a record that is only tool-results contributes nothing."""
    msgs = []
    for r in recs:
        if r.get("type") != "user":
            continue
        c = _content(r)
        if isinstance(c, list):
            texts = [b.get("text", "") for b in c
                     if isinstance(b, dict) and b.get("type") == "text" and b.get("text", "") != ""]
            if not texts:
                continue
            msgs.append(" ".join(texts))
        elif isinstance(c, str):
            msgs.append(c)
        else:
            msgs.append("")
    return "\n".join(msgs[-n:])


_NOISE_PREFIXES = ("<", "[", "Base directory", "This session is being continued")


def conversation_stream(recs, user_cap=200, assistant_cap=300):
    """Background `U|`/`A|` stream for intent classification: visible user messages
    (scalar string content, system-noise prefixes skipped) and assistant text, each
    bounded by the caller's per-line cap. Returns a list of lines.

    The classifier runs on every prompt, so it passes tight caps on purpose — raise
    them only if the per-turn latency is acceptable."""
    lines = []
    for r in recs:
        etype = r.get("type")
        c = _content(r)
        if etype == "user" and isinstance(c, str):
            if c.startswith(_NOISE_PREFIXES):
                continue
            lines.append("U|" + clamp(c.replace("\n", " "), user_cap))
        elif etype == "assistant" and isinstance(c, list):
            texts = [b.get("text", "") for b in blocks(r, "text")]
            if texts:
                lines.append("A|" + clamp(" ".join(texts).replace("\n", " "), assistant_cap))
    return lines
