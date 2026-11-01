"""Shared Claude transcript layer.

Locates, reads, and parses Claude Code's JSONL transcript and exposes the views
hooks compose from — records, message blocks, the current turn, who spoke, and
Skill arrivals — so each hook stops hand-rolling its own line-by-line parse.
"""

import json
import os
import re


_CHUNK = 1 << 20


def records(path):
    """The parsed records still in the conversation, chronological; bad lines skipped.

    A compaction leaves the old records in the file while dropping them from the
    conversation, so a long session's file holds every window it ever had. The
    file is read from its end back to the newest compaction summary, and the
    cost follows the live conversation, never the session's whole history. At
    `SessionStart: compact` the new summary is not in the file yet, so this
    returns the whole window that just closed.
    """
    if not path or not os.path.isfile(path):
        return []
    out = []
    try:
        for line in _lines_from_end(path):
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except ValueError:
                continue
            if text_of(record).startswith(COMPACT_MARKER):
                break
            out.append(record)
    except OSError:
        return []
    out.reverse()
    return out


def _lines_from_end(path):
    """The file's lines as text, last line first.

    The file is read backward one chunk at a time, and a line cut by a chunk's
    start waits for the next chunk. A newline byte never occurs inside a UTF-8
    character, so no character is split either.
    """
    with open(path, "rb") as fh:
        end = fh.seek(0, os.SEEK_END)
        head = b""
        while end > 0:
            start = max(0, end - _CHUNK)
            fh.seek(start)
            lines = (fh.read(end - start) + head).split(b"\n")
            head = lines.pop(0)
            end = start
            for line in reversed(lines):
                yield line.decode("utf-8", errors="replace")
        yield head.decode("utf-8", errors="replace")


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


def _queued(record):
    """The `queued_command` attachment a Claude record carries, or None.

    A message that arrives while a turn is running is never written as a user
    record. Claude absorbs it into the running turn and records it as this
    attachment, labelled with its origin."""
    attachment = record.get("attachment") if record.get("type") == "attachment" else None
    return attachment if isinstance(attachment, dict) and attachment.get("type") == "queued_command" else None


def text_of(record):
    """A record's text: scalar content, or its text blocks joined.

    Handles both transcript shapes. Claude puts content on `message`; codex puts
    it on `payload`, with `input_text`/`output_text` blocks instead of `text`."""
    queued = _queued(record)
    if queued:
        prompt = queued.get("prompt")
        return prompt if isinstance(prompt, str) else ""
    c = _content(record)
    if c is None:
        c = (record.get("payload") or {}).get("content")
    if isinstance(c, str):
        return c
    if not isinstance(c, list):
        return ""
    parts = [b.get("text", "") for b in c
             if isinstance(b, dict) and b.get("type") in ("text", "input_text", "output_text")]
    return "\n\n".join(p for p in parts if p)


# What a harness-authored block looks like when nothing labels it: a whole message
# that is one closed XML tag, one bracketed line, or a known machine preamble.
# Codex needs this for its `<recommended_plugins>` and `<environment_context>`
# injections, which arrive on the user role carrying no metadata at all, and the
# intent classifier runs the same test on a raw prompt.
# The record a Skill's own arrival writes, named because two readers key on it:
# the machine-authored test below, and `skill_arrivals`.
SKILL_PREAMBLE = "Base directory for this skill:"
# The summary the harness writes where it compacted the conversation: everything
# before it is out of the model's sight.
COMPACT_MARKER = "This session is being continued"

_MACHINE_PREAMBLES = (
    COMPACT_MARKER,
    SKILL_PREAMBLE,
    "Stop hook feedback:",
    "Another Claude session sent a message:",
)


def harness_authored(text):
    """Whether text looks machine-authored on its face, with no label to read."""
    text = (text or "").strip()
    if not text:
        return False
    if text.startswith("<"):
        tag = re.split(r"[> \n]", text[1:], maxsplit=1)[0]
        if tag and ("</%s>" % tag) in text:
            return True
    if text.startswith("["):
        first = text.split("\n", 1)[0]
        if first.endswith("]") and text == first:
            return True
    return text.startswith(_MACHINE_PREAMBLES)


def current_turn(recs):
    """Records after the last genuine user message, chronological."""
    cut = 0
    for i, r in enumerate(recs):
        if is_real_user(r):
            cut = i + 1
    return recs[cut:]


# The tag reload_stale_skills wraps its orders in, and the one sentence shape an
# order takes: `Use /a, /b now`. Only that sentence names what was ordered — the
# same block mentions other Skills in passing, such as `research with /discover`.
RELOAD_TAG = "<reload_stale_skills_agent>"
_ORDER = re.compile(r"\bUse ((?:/[a-z0-9][a-z0-9-]*(?:, )?)+) now\b")
_SKILL_NAME = re.compile(r"/([a-z0-9][a-z0-9-]*)")


def ordered_skills(record):
    """The Skills a reload_stale_skills order named, or [] for any other record.

    The harness stores a hook's context as an `attachment` record with the text
    under `attachment.content`, one string per hook. Only the refresher's own tag
    counts: classify_intent orders Skills too, and those are a typed command
    expanding in the same turn, which `skill_arrivals` already sees.
    """
    if record.get("type") != "attachment":
        return []
    attachment = record.get("attachment") or {}
    if attachment.get("type") != "hook_additional_context":
        return []
    out = []
    for text in attachment.get("content") or []:
        if isinstance(text, str) and RELOAD_TAG in text:
            for named in _ORDER.findall(text):
                out.extend(_SKILL_NAME.findall(named))
    return out


def skill_arrivals(recs, orders=True):
    """Skill name -> index of the record that last brought it into the conversation.

    Three ways in. The architect types /<name> and the harness expands the Skill
    itself, writing the `Base directory for this skill:` record
    `_MACHINE_PREAMBLES` already knows. The agent uses the Skill through the Skill
    tool, whose call stays in the transcript whatever the tool answers — and a
    second use answers `instructions unchanged`, so the call is the only record of
    it. And reload_stale_skills orders one, which counts so that an overdue Skill is
    ordered once per distance rather than before every model request; pass
    `orders=False` to see only the uses, which is what "in use" means.

    A Skill absent here was never used, which is what makes this the whole answer
    to which Processes govern: no list is kept beside it.
    """
    out = {}
    for i, record in enumerate(recs):
        text = text_of(record)
        if text.startswith(SKILL_PREAMBLE):
            out[os.path.basename(text.splitlines()[0].rstrip())] = i
            continue
        if orders:
            for name in ordered_skills(record):
                out[name] = i
        for block in blocks(record, "tool_use"):
            if block.get("name") == "Skill":
                name = (block.get("input") or {}).get("skill")
                if isinstance(name, str) and name:
                    out[name] = i
    return out


def user_rules_loaded(recs):
    """The paths of the user Rules Claude Code loaded in these records, in load order.

    Claude Code does not load a user Rule again after a compaction once the session
    has loaded it, while it does reload project docs.
    """
    out = []
    for record in recs:
        attachment = record.get("attachment") or {}
        loaded = attachment.get("content") or {}
        path = attachment.get("path")
        if (attachment.get("type") == "nested_memory" and loaded.get("type") == "User"
                and path and path not in out):
            out.append(path)
    return out
