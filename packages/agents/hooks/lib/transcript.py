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
import re

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


def latest_user_record(recs):
    """The most recent genuine user record, or None.

    On a UserPromptSubmit the harness has already written the prompt, so this is
    the record for the prompt the hook is firing on."""
    for r in reversed(recs):
        if is_real_user(r):
            return r
    return None


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

_MACHINE_PREAMBLES = (
    "This session is being continued",
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


def is_architect(record, meta=None):
    return speaker(record, meta) == Architect


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


def architect_request(recs):
    """The architect's most recent message, however many records came after it.

    `architect_message` answers a different question — "is the prompt this hook
    fired on his?" — and stops at the newest user record, because at
    UserPromptSubmit that record is the prompt. At Stop it is a tool result, a
    loaded skill, or an injected block, and stopping there returns "" on nearly
    every turn that used a tool. A gate judging a reply needs the ask the reply
    answers, so this one keeps walking."""
    meta = session_meta(recs)
    for r in reversed(recs):
        if _role(r) == "user" and speaker(r, meta) == Architect:
            return text_of(r)
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


def current_turn_lines(path):
    """Raw JSONL lines of the current turn, chronological.

    The stop gate scans the turn's raw line text for substrings (`"name":"Edit"`),
    deliberately looser than parsed semantics, so it needs the original bytes, not
    re-serialized records. Boundary matches current_turn: walk from the end, stop
    (exclusive) at the first genuine user line (`"type":"user"` without
    `tool_use_id`)."""
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


def awaits_async_work(lines):
    """Whether the turn's raw lines dispatched work that finishes after the stop.

    Two Stop gates yield on this: stopping to await a dispatch is an async pause,
    not skipped work, because the dispatch wakes the agent when it lands.

    Two substrates, two signals. A `codex-run` is a Bash call carrying
    `run_in_background: true`. An Agent dispatch has no such flag — the tool takes
    no parameter for it and is async in every case — so the tool call itself is
    the signal. Keying only on the flag gated every turn that awaited a Subagent.
    """
    for line in lines:
        packed = line.replace(" ", "")
        if '"run_in_background":true' in packed or '"name":"Agent"' in packed:
            return True
    return False


def skill_arrivals(recs):
    """Skill name -> index of the record that last brought it into the conversation.

    Two ways in, because there are two ways to ask for one. The architect types
    /<name> and the harness expands the Skill itself, writing the
    `Base directory for this skill:` record `_MACHINE_PREAMBLES` already knows.
    The agent uses the Skill through the Skill tool, whose call stays in the
    transcript whatever the tool answers — and a second use answers
    `instructions unchanged`, so the call is the only record of it.

    A Skill absent here was never used, which is what makes this the whole answer
    to which Processes govern: no list is kept beside it.
    """
    out = {}
    for i, record in enumerate(recs):
        text = text_of(record)
        if text.startswith(SKILL_PREAMBLE):
            out[os.path.basename(text.splitlines()[0].rstrip())] = i
            continue
        for block in blocks(record, "tool_use"):
            if block.get("name") == "Skill":
                name = (block.get("input") or {}).get("skill")
                if isinstance(name, str) and name:
                    out[name] = i
    return out


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


def edited_paths(turn_recs):
    """Distinct file paths the turn edited via the edit tools, in first-seen order.

    The blind-edit completion check reads this to learn which files the turn
    changed; read-coverage and callers are then looked up per path. Mirrors
    `turn_evidence`'s edit-tool iteration — same EDIT_TOOLS set, same
    file_path/notebook_path target extraction."""
    out, seen = [], set()
    for r in turn_recs:
        if r.get("type") != "assistant":
            continue
        for b in blocks(r, "tool_use"):
            if b.get("name") not in EDIT_TOOLS:
                continue
            inp = b.get("input") or {}
            path = inp.get("file_path") or inp.get("notebook_path") or ""
            if path and path not in seen:
                seen.add(path)
                out.append(path)
    return out


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


def conversation_stream(recs, user_cap=200, assistant_cap=300):
    """Background `U|`/`A|` stream for intent classification: what the architect
    said and what the agent answered, each bounded by the caller's per-line cap.
    Returns a list of lines.

    User lines are selected by provenance, not by shape. Reading `promptSource`
    keeps a pasted `<config>` block or an `[Image #1]` message — both the
    architect's — in the stream, where a prefix test dropped them as noise.

    The classifier runs on every prompt, so it passes tight caps on purpose — raise
    them only if the per-turn latency is acceptable."""
    meta = session_meta(recs)
    lines = []
    for r in recs:
        etype = r.get("type")
        c = _content(r)
        if etype == "user" and is_architect(r, meta):
            text = text_of(r)
            if text:
                lines.append("U|" + clamp(text.replace("\n", " "), user_cap))
        elif etype == "assistant" and isinstance(c, list):
            texts = [b.get("text", "") for b in blocks(r, "text")]
            if texts:
                lines.append("A|" + clamp(" ".join(texts).replace("\n", " "), assistant_cap))
    return lines


def conversation_context(path):
    """Formatted background block for the every-prompt classifiers: the recent
    conversation, then the agent's last response (what the latest user message is
    replying to). Empty string when the transcript yields nothing. Both
    UserPromptSubmit model calls — intent and goal — build their context from this."""
    stream_lines = conversation_stream(records(path))
    if not stream_lines:
        return ""

    turn_line = 0
    for idx, line in enumerate(stream_lines, start=1):
        if line.startswith("U|"):
            turn_line = idx

    recent_turns = ""
    agent_response = ""
    if turn_line > 0:
        after = stream_lines[turn_line:]
        agent_blocks = [ln[2:] for ln in after if ln.startswith("A|")]
        agent_response = "\n".join(agent_blocks[-5:])
        if turn_line > 1:
            before = stream_lines[:turn_line - 1]
            recent = before[-8:]
            recent_turns = "\n".join(
                ("[User] " + ln[2:]) if ln.startswith("U|")
                else ("[Agent] " + ln[2:]) if ln.startswith("A|")
                else ln
                for ln in recent
            )
    else:
        agent_blocks = [ln[2:] for ln in stream_lines if ln.startswith("A|")]
        agent_response = "\n".join(agent_blocks[-5:])

    if not recent_turns and not agent_response:
        return ""
    parts = ""
    if recent_turns:
        parts = "Recent conversation (background):\n%s\n\n---\n" % recent_turns
    if agent_response:
        parts = ("%sAgent's last response (what the user is responding to):\n%s\n\n---\n"
                 % (parts, agent_response))
    return parts
