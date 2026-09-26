"""Shared Claude transcript layer.

Locates, reads, and parses Claude Code's JSONL transcript and exposes the views
hooks compose from — records, message blocks, the current turn, who spoke, and
Skill arrivals — so each hook stops hand-rolling its own line-by-line parse.
"""

import json
import os
import re


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


def text_of(record):
    """A record's text: scalar content, or its text blocks joined.

    Handles both transcript shapes. Claude puts content on `message`; codex puts
    it on `payload`, with `input_text`/`output_text` blocks instead of `text`."""
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


# --- who spoke -----------------------------------------------------------------
#
# One vocabulary for the author of a message, in our words. Every record resolves
# to exactly one of three, and this module is the single owner of the translation
# — the way lib/event.py owns tool names. Nothing above it names a vendor field.
#
#   Architect — he typed it
#   Agent     — the model wrote it
#   Harness   — the tool put it on the wire: task notifications, injected context,
#               environment and plugin blocks, skill loads, compaction notices
#
# Capitalized as Domain.md writes them, because they are those words, not new
# ones. Screaming caps would read as configuration constants; these are the
# project's nouns.
#
# The harnesses disagree about where authorship is recorded, never about what it
# means. Claude Code multiplexes all three onto one user channel inside one
# session, so it labels every record. Codex gives a thread one entry point for its
# whole life, so it labels the thread once and leaves its own injected blocks
# unlabelled — there, shape is the only evidence left.
#
# Unknown denies. A record that resolves to nothing is never the architect, so a
# label either harness adds later stays out of his memory until it is admitted
# here.

Architect = "architect"
Agent = "agent"
Harness = "harness"

# Claude's `promptSource`, keyed onto ours. `queued` is him typing while a turn
# was still running.
_CLAUDE_AUTHOR = {"typed": Architect, "queued": Architect, "system": Harness}

# Codex names its thread's entry point instead. Only a person at the desktop app
# is him: `codex-run` is this repo's own wrapper, `codex_exec` is a script, and a
# `source` that is a dict carries a subagent spawn.
_ARCHITECT_ORIGINATORS = ("codex_work_desktop",)

# What a harness-authored block looks like when nothing labels it: a whole message
# that is one closed XML tag, one bracketed line, or a known machine preamble.
# Codex needs this for its `<recommended_plugins>` and `<environment_context>`
# injections, which arrive on the user role carrying no metadata at all. On Claude
# it is a backstop behind the label, and the same test the intent classifier runs
# on a raw prompt.
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


def session_meta(recs):
    """A codex rollout's opening `session_meta` payload; {} for a Claude one."""
    for r in recs:
        if r.get("type") == "session_meta":
            return r.get("payload") or {}
    return {}


def _role(record):
    """The record's role in either shape."""
    kind = record.get("type")
    if kind in ("user", "assistant"):
        return kind
    return (record.get("payload") or {}).get("role", "")


def speaker(record, meta=None):
    """Who authored this record: Architect, Agent, Harness, or "" when unknown.

    `meta` is the transcript's `session_meta`, which only codex writes; pass it so
    a codex record can be resolved against its thread's entry point."""
    role = _role(record)
    if role == "assistant":
        return Agent
    if role != "user":
        return ""
    if record.get("isMeta"):
        return Harness
    if meta:
        if not isinstance(meta.get("source"), str):
            return Agent
        if meta.get("originator") not in _ARCHITECT_ORIGINATORS:
            return Agent
        return Harness if harness_authored(text_of(record)) else Architect
    author = _CLAUDE_AUTHOR.get(record.get("promptSource"), "")
    if author == Architect and harness_authored(text_of(record)):
        return Harness
    return author


def architect_message(recs):
    """The architect's most recent message in this transcript, or ""."""
    meta = session_meta(recs)
    for r in reversed(recs):
        if _role(r) != "user":
            continue
        if speaker(r, meta) == Architect:
            return text_of(r)
        if not meta:
            # Claude labels every record, so the newest user record is the prompt
            # this fired on; an older one is a different turn, not this one.
            return ""
    return ""


def agent_replies(recs):
    """The agent's replies in the current turn, joined, for either transcript.

    Codex has no user/assistant turn boundary this can key on the way Claude's
    `current_turn` does, so a codex thread contributes its whole reply stream; the
    hook fires once per turn either way."""
    scope = recs if session_meta(recs) else current_turn(recs)
    return "\n\n".join(t for t in (text_of(r) for r in scope
                                   if speaker(r) == Agent) if t.strip())


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


def live_records(recs):
    """The records still in the conversation: everything after the last compaction.

    A compaction leaves the old records in the transcript file while dropping them
    from the conversation. At `SessionStart: compact` the new boundary is not in
    the file yet, so this returns the whole window that just closed.
    """
    cut = 0
    for i, record in enumerate(recs):
        if text_of(record).startswith(COMPACT_MARKER):
            cut = i + 1
    return recs[cut:]


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
