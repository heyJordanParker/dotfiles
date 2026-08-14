#!/usr/bin/env python3
"""Judge the reply the architect is about to read (Stop).

The one Stop gate. It owns the LAST MESSAGE on every axis: whether it is worth his
time — self-contained, concrete, structured, clearly worded, scope-faithful, free
of anything he must remember — whether the work it reports is finished, and
whether the changes it reports are grounded in the code they touched.

Two gates used to split that judgement and each guessed at the other's half, which
is what made them duplicate and contradict. One call sees the whole turn instead.

It reads the architect's own last message, resolved by speaker so a loaded skill
or an injected block is never judged as his ask, and the agent's reply. Nothing
else: correctness of the reply is judged from the reply, not the session.

The Stop event's Rules, batched into one model call, and the turn's own facts pick
which ones it carries — a Rule the turn cannot produce never reaches the judge,
rather than shipping with a Condition the judge has to apply to itself. Intent
comes off the spine where classify_intent stored it, mode and state off the same
spine.

allow=false raises a non-halting concern carrying the offending Rule; the response
contract that concern arrives with belongs to feedback.raise_concern's Stop
channel, not to any Rule here. Any infrastructure failure (recursion guard, model
unavailable, parse error) returns 0 and never blocks — a broken model must not
wedge the agent.
"""

import json
import os
import re
import shutil
import subprocess
import sys

from lib import feedback, transcript
from lib.event import field, owner_session, read_event
from lib.model_call import run_model
from lib.session_mode import is_dispatched, permits, resolve, state
from lib.session_state import bump_gate_block, gate_block_count, load_state

# This gate blocks at most this many times per turn, then yields for the rest of
# the turn — it flags an obvious problem once and trusts the agent after that.
GATE_BLOCK_CAP = 1

# Below this read-fraction, an edited file counts as barely-read. The tracer
# records an edit's PreToolUse shoulder as a whole-file read, so an edited file's
# coverage is usually near 1.0; the floor catches the residual
# partial-read-then-edit case and a file that never recorded any read at all.
COVERAGE_FLOOR = 0.5

# Claude only. The judgement is built on Claude's transcript — the turn boundary,
# the request, the tool evidence — and codex's rollout carries none of those
# shapes, so on codex it would judge a bare last message with no request and no
# evidence, at the cost of a model call per stop.
BINDING = {
    "events": {"Stop": []},
    "timeout": 90,
    "harness": "claude",
}

JSON_SCHEMA = ('{"type":"object","properties":{"allow":{"type":"boolean"},'
               '"reason":{"type":"string"}},'
               '"required":["allow"]}')

SYSTEM_PROMPT = (
    "You are a babysitter for an AI agent's reply to a software architect. The "
    "architect has the WHOLE conversation in mind: anything already used, named, or "
    "discussed this session — a file, a term, an abbreviation, a shorthand — is known "
    "to him and needs no re-explaining, re-pathing, or restating. He reads the last "
    "message and acts on it; he will not re-read files or dig up code. You have "
    "LIMITED context — only the last message and his last message — so you are a "
    "limited check, not the orchestrator: flag only what is obviously wrong in the "
    "message itself, never prescribe process, and when a block would need context you "
    "don't have, or you are unsure, ALLOW. You see only a slice — the last message "
    "and a little session data — never the full state. You do not know what earlier "
    "turns already finished, whether a background command or measurement the agent "
    "started has completed, or whether a listed requirement is still open. NEVER invent "
    "that state to justify a block: if a block needs state you were not given, ALLOW. A "
    "message reporting it started background work and will return when it lands is a "
    "legitimate wait, not a stale or deferring reply — never block it for that. "
    "Block ONLY when the architect would be "
    "genuinely confused or actively misled — never to make an already-clear message "
    "tighter, more complete, or better-cited. Missing brackets, a dropped file path, "
    "or shorthand whose meaning is plain from the conversation are polish, not "
    "confusion: ALLOW. The test for every rule below: without your block, would the "
    "architect get a response he cannot use — one he would have to send back for a "
    "rewrite? If yes, block. If the response is already usable and clear, ALLOW it even "
    "when imperfect — never block to trim a word, tighten a phrase, remove a harmless "
    "line, reorder for a marginal gain, or make a usable reply 2% better. A wrong block "
    "costs the architect an entire extra turn and real money; a clear-but-imperfect "
    "message costs nothing. But permissiveness protects "
    "clear, honest, complete messages — it NEVER protects a confident wrong fact or a "
    "punt. A reply that states a fact about the architect's own stack, code, or data the "
    "agent never verified, hands him a decision or question the agent could have settled "
    "by reading the code, recommends something whose effect contradicts the goal it "
    "serves, or answers with a shortlist of directions where he asked for the exact "
    "changes is not polish — it actively misleads or wastes his time, and you block it. "
    "The architect reads architecture "
    "— the purpose of files, who owns which data, the public API, the exact database "
    "changes — and he reads the documentation his agents depend on (prose, rules, "
    "prompts, ADRs) including its diffs. What he does not read is program-code source "
    "pasted in place of an architectural account of a change. Never demand inlined "
    "code or line-number citations. The agent naming the files, the public API, and exact cited "
    "values is evidence it read the code — never demand a re-scan, re-research, or "
    "that it re-deliver a list or answer it already gave and the architect already "
    "responded to. You only RAISE the concern in `reason`; you never prescribe a "
    "fix, an edit, or a next step — the main agent has the full context and owns that. "
    "Output structured JSON only."
)


PERMISSION_PHRASES = (
    "shall i proceed",
    "shall i continue",
    "want me to continue",
    "let me continue in the next message",
    "should i move on",
    "ready to proceed",
    "can i proceed",
    "ready to move",
    "want me to go ahead",
    "should i proceed",
)

FORWARDED_PHRASES = (
    "the agent recommends",
    "the agent recommended",
    "the subagent recommends",
    "the subagent recommended",
    "the teammate recommends",
    "the teammate recommended",
    "per the research",
    "per the subagent",
    "per the agent",
    "per the teammate",
    "based on the findings,",
    "following the analysis,",
    "architect recommended option",
    "architect recommends option",
    "researcher recommended option",
    "researcher recommends option",
    "reviewer recommended option",
    "reviewer recommends option",
    "engineer recommended option",
    "engineer recommends option",
)

UNSETTLED_PHRASES = (
    "i have not checked",
    "i haven't checked",
    "i have not verified",
    "i did not verify",
    "i could not verify",
    "i have not confirmed",
    "i cannot confirm",
)

PLAN_APPROVED_MSG = "Potential issue: the plan is approved, but the message asks permission to start the approved work."


def _strip_markdown(text):
    """Strip markdown quote constructs so the forwarded-recommendation gate
    doesn't false-fire on test output, doc snippets, examples, or user-quote
    echoes. Fenced blocks (line-structural) and blockquote/indented lines are
    dropped whole; inline code and quoted spans are stripped within surviving
    lines. Bold and italic stay intact — those mark the agent's own assertions,
    not quotes."""
    out = []
    in_fence = False
    for line in text.split("\n"):
        if line.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if line.startswith(">"):
            continue
        if line.startswith("    "):
            continue
        line = re.sub(r"`[^`]*`", "", line)
        line = re.sub(r'"[^"]*"', "", line)
        line = re.sub(r"'[^']*'", "", line)
        out.append(line)
    return "\n".join(out)


# --- Read-log facts -----------------------------------------------------------
#
# The tracer's session log records, per file, how much of it the agent read this
# session — surfaced by `trace docs status`. A read counts no matter how it
# happened: the builtin Read/Edit tools, a `trace read`/`context` call, and a
# shell `cat` all flow through the same `trace context` shoulder into the same
# accumulator. The caller graph (`trace info` top_callers) names the files that
# depend on an edited file. The Rules reason over these facts; the code only
# gathers them.


def _trace_env(session_id):
    """A copy of the environment carrying this session's id for the tracer, the
    same way enrich_on_read / inject_docs hand trace their run's session. Never
    mutates os.environ. AGENT_SESSION_ID is the harness-neutral carrier trace
    resolves first, so `trace docs status` reads this session's read log."""
    env = dict(os.environ)
    if session_id:
        env["AGENT_SESSION_ID"] = session_id
    return env


def _read_log_status(cwd, env):
    """`trace docs status --json` for this session, projected to the two facts the
    Rules need: (coverage, opened). coverage maps a read file's realpath ->
    read_fraction; opened is the set of realpaths the agent actually read this
    session (kind read_file — doc-injected and directory entries are not "opened
    to edit"). Empty pair on any failure, so the Rules degrade to silent."""
    try:
        r = subprocess.run(["trace", "docs", "status", "--json"],
                           cwd=cwd, env=env, text=True, capture_output=True, timeout=10)
    except Exception:
        return {}, set()
    if r.returncode != 0:
        return {}, set()
    try:
        loaded = json.loads(r.stdout).get("loaded", []) or []
    except Exception:
        return {}, set()
    coverage, opened = {}, set()
    for entry in loaded:
        if entry.get("kind") != "read_file":
            continue
        rp = os.path.realpath(entry.get("path", ""))
        opened.add(rp)
        coverage[rp] = entry.get("read_fraction", 0.0)
    return coverage, opened


def _warm_graph(cwd, env):
    """Rebuild and persist the architecture graph for the current (dirty) tree.

    At stop time the agent has just edited files, so the architecture cache is
    stale — and `trace info`'s caller lookup reads the cache load-only, returning
    no callers against a stale entry. `trace status` is the one dirty-tree command
    that rebuilds the graph and persists it, so a following `trace info` in a
    fresh process validates that entry and serves real callers. Run once for its
    side effect before any caller lookup; output discarded."""
    try:
        subprocess.run(["trace", "status", "--json"],
                       cwd=cwd, env=env, capture_output=True, timeout=20)
    except Exception:
        pass


def _caller_files(file_path, cwd, env):
    """Repo-relative source files that directly call the edited file, via
    `trace info <file>` top_callers (up to ten direct callers). Reads the graph
    load-only, so the caller must `_warm_graph` first against a dirty tree. Empty
    on any failure — the Rule then sees a file with no known callers."""
    try:
        r = subprocess.run(["trace", "info", file_path, "--json"],
                           cwd=cwd, env=env, text=True, capture_output=True, timeout=10)
    except Exception:
        return []
    if r.returncode != 0:
        return []
    try:
        info = json.loads(r.stdout)
    except Exception:
        return []
    return [c["source_file"] for c in (info.get("top_callers") or [])
            if isinstance(c, dict) and c.get("source_file")]


def _edit_fact_line(file_path, fraction, callers, opened):
    """One fact line for an edited file, and whether it is a blind-edit risk.

    Pure: given the file's read-fraction (None when no read was recorded), its
    caller source files, and the session's opened-realpath set, render the line
    the judge reads and decide whether this edit is unhardened — barely-read OR
    no caller was read. A caller is "read" when an opened path ends with its
    repo-relative source file."""
    read_callers = sum(1 for c in callers
                       if any(o.endswith(os.sep + c) for o in opened))
    if fraction is None:
        cov_txt = "no read recorded"
    else:
        cov_txt = "%.0f%% read" % (fraction * 100)
    if callers:
        callers_txt = "%d caller file(s), %d read" % (len(callers), read_callers)
    else:
        callers_txt = "no caller files in the graph"
    line = "- %s: %s; %s" % (file_path, cov_txt, callers_txt)
    barely_read = fraction is None or fraction < COVERAGE_FLOOR
    unread_callers = bool(callers) and read_callers == 0
    return line, (barely_read or unread_callers)


def _edited_facts(edited_files, coverage, opened, cwd, env):
    """Per-edited-file fact lines plus whether any edit is a blind-edit risk."""
    _warm_graph(cwd, env)
    lines, risk = [], False
    for f in edited_files:
        fraction = coverage.get(os.path.realpath(f))
        line, file_risk = _edit_fact_line(f, fraction, _caller_files(f, cwd, env), opened)
        lines.append(line)
        risk = risk or file_risk
    return "\n".join(lines), risk


# A path-shaped token in the message: a slash-joined name with an extension, or a
# bare filename with one. The extension must be lowercase and at least two
# characters, which is what keeps prose out — "e.g", "i.e", and a missing space
# after a full stop ("the file.The next step") all read as filenames otherwise,
# and each would be reported as a file the message proposes creating.
_PATH_TOKEN = re.compile(r"(?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+\.[a-z][a-z0-9]{1,7}\b"
                         r"|\b[A-Za-z0-9_-]+\.[a-z][a-z0-9]{1,7}\b")


def _missing_paths(text, cwd):
    """Path-shaped tokens in the message that do not exist on disk, first-seen order.

    A file the message proposes CREATING can never appear in the opened set, so
    without this fact the unopened-edit Rule reads a new file as code the agent
    changed blind."""
    out, seen = [], set()
    for token in _PATH_TOKEN.findall(text or ""):
        if token in seen:
            continue
        seen.add(token)
        candidates = [token, os.path.join(cwd, token)] if cwd else [token]
        if not any(os.path.exists(c) for c in candidates):
            out.append(token)
    return out


def _exited_plan_mode(turn_recs):
    """Whether the turn actually called ExitPlanMode.

    Read from the parsed tool_use blocks, never from the raw line text: the word
    appears in this hook's own rendered prompt, which lands back in the transcript
    whenever a turn prints it, and a substring scan then reports an approved plan
    that never existed."""
    for r in turn_recs:
        if r.get("type") != "assistant":
            continue
        for b in transcript.blocks(r, "tool_use"):
            if b.get("name") == "ExitPlanMode":
                return True
    return False


def _dispatched_this_session(recs):
    """Whether any subagent was dispatched in this session.

    A subagent's reads are logged under its own agent id, never under the
    session's, so `trace docs status` here reports a session that delegated its
    reading as one that read almost nothing."""
    for r in recs:
        if r.get("type") != "assistant":
            continue
        for b in transcript.blocks(r, "tool_use"):
            if b.get("name") == "Agent":
                return True
    return False


def _read_facts(edited_facts_block, opened, missing):
    """The facts the reading Rules reason over: per-edited-file coverage and
    caller lines, the opened-files list, then the paths the message names that do
    not exist. Empty string when there is nothing to say."""
    parts = ""
    if edited_facts_block:
        parts += ("Files this turn EDITED, with read-coverage and caller-read facts:"
                  "\n%s\n---\n" % edited_facts_block)
    if opened:
        parts += ("Files the agent OPENED (read) this session — every other file is "
                  "unopened:\n%s\n---\n" % "\n".join(sorted(opened)))
    if missing:
        parts += ("Paths the message names that do not resolve from this session's "
                  "working directory. Treat each as a file the message proposes "
                  "CREATING, so no Rule about unopened or unread files applies to it. "
                  "This list is not evidence about the repository: a path resolves "
                  "from one directory and not another, so absence here never disproves "
                  "a claim the message makes:\n%s\n---\n" % "\n".join(missing))
    return parts


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or is_dispatched(event):
        return 0

    mode = resolve(event)
    if mode == "interview":
        return 0
    last_msg = field(event, "last_assistant_message", "")
    if not last_msg.strip():
        return 0

    # Already flagged this turn — yield rather than re-block the same stop, which
    # is what freezes the session.
    if gate_block_count(session_id, "babysitter") >= GATE_BLOCK_CAP:
        return 0

    transcript_path = field(event, "transcript_path", "")
    recs = transcript.records(transcript_path)
    turn = transcript.current_turn(recs)
    # The governing session, not this process's own: a subagent reads its parent's
    # axes, and a codex run launched by Claude reads the launcher's. `state` already
    # resolves that session, so intent and the commit flag are read from the same
    # record — split across two sessions they can disagree about one turn.
    spine = load_state(owner_session(event))
    current_state = state(event)

    # The one gate that decides alone, ahead of any model call. It rules on wording
    # plus a fact from the parsed records, and that second fact is what makes it
    # safe: an approved plan is proved by the ExitPlanMode call, never by the words.
    # It reads parsed records, never the raw line text, because `ExitPlanMode`
    # appears in this hook's own rendered prompt, which lands back in the transcript.
    #
    # Every other matcher below only reports that some characters appeared. Whether
    # those characters are a failure depends on who the sentence is about and what
    # the architect asked for, which the judge reads and a substring cannot. So they
    # travel into the call as facts, carrying the matched wording, and the judge
    # rules. A matcher ruling alone on wording blocks good replies, and a wrong block
    # costs the architect a whole turn.
    permission = next((p for p in PERMISSION_PHRASES if p in last_msg.lower()), "")
    if permission and _exited_plan_mode(turn):
        return feedback.raise_concern("babysitter", "Stop", PLAN_APPROVED_MSG)

    stripped = _strip_markdown(last_msg).lower()
    forwarded = next((p for p in FORWARDED_PHRASES if p in stripped), "")
    unsettled = next((p for p in UNSETTLED_PHRASES if p in stripped), "")

    # Already flagged this turn — yield rather than re-block the same stop, which
    # is what freezes the session. The deterministic gates above still fire every
    # turn; only the judgement is capped.
    if gate_block_count(session_id, "babysitter") >= GATE_BLOCK_CAP:
        return 0

    # Stopping to await dispatched work is an async pause, not skipped work — the
    # agent resumes when the dispatch wakes it. Don't gate that.
    if transcript.awaits_async_work(transcript.current_turn_lines(transcript_path)):
        return 0

    # Wrapping up: the commit is the deliverable, not the message.
    if spine.get("commit_requested"):
        return 0

    # The architect's own last message, resolved by speaker. The newest `user`
    # record is very often not him — a loaded skill, an injected hook block, this
    # gate's own feedback replayed back — and judging the reply against one of
    # those is how a question he did ask reads as unanswered.
    request = transcript.clamp(transcript.architect_request(recs))
    intent = spine.get("intent") or "action"
    # The state axis holds a main session's writes back while it proposes; mode
    # holds a dispatched or orchestrating agent's. Either way the agent could not
    # have edited or run a check this turn.
    can_write = permits(event, "write") and current_state == "execute"

    cwd = field(event, "cwd", "") or os.getcwd()
    coverage, opened, edited_facts_block = {}, set(), ""
    if shutil.which("trace"):
        env = _trace_env(session_id)
        coverage, opened = _read_log_status(cwd, env)
        edited_files = transcript.edited_paths(turn)
        if edited_files:
            edited_facts_block, _ = _edited_facts(
                edited_files, coverage, opened, cwd, env)
    # A delegating session's reads live under its subagents' ids, so the opened set
    # is not evidence of what this agent knows. Drop it, and the Rules that reason
    # over it drop with it.
    if _dispatched_this_session(recs):
        opened = set()

    prompt = _eval_prompt(
        request, last_msg, intent, current_state, mode, can_write,
        transcript.clamp(transcript.plan_content(recs)),
        transcript.clamp(transcript.turn_evidence(turn)),
        _read_facts(edited_facts_block, opened, _missing_paths(last_msg, cwd)),
        permission, forwarded, unsettled, bool(opened), bool(edited_facts_block),
    )
    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=prompt,
                       schema=JSON_SCHEMA)
    if not result:
        return 0

    if result.get("allow", True) is False:
        reason = result.get("reason") or "the reply may not be optimized for the architect"
        bump_gate_block(session_id, "babysitter")
        return feedback.raise_concern("babysitter", "Stop", "Potential issue: %s" % reason)

    return 0


INVENTED_TERM = (
    "### Name an invented term he cannot decode\n"
    "The message uses a term the agent invented for something the project already "
    "names, and that term is opaque: its meaning is not clear from the message or the "
    "conversation, as \"hid\" is for \"hidden: true\" on first use. Name the term. A "
    "term, abbreviation, file nickname, or shorthand already used this session, or "
    "whose meaning is obvious in context, is one he knows: allow it, and never demand "
    "a full path for a file already discussed. The project's own vocabulary always "
    "passes, and pasted code is never the fix."
)

COLLIDING_TERMS = (
    "### Name two words used for one thing\n"
    "The message uses different words for one thing, or one word for two things, so he "
    "has to work out which is meant. Name the colliding terms."
)

TWO_READINGS = (
    "### Quote a sentence that carries two readings\n"
    "A sentence carries more than one reading, or its meaning is unclear. Quote that "
    "sentence."
)

CATEGORY_NOT_CHANGE = (
    "### Name a change given only as a category\n"
    "The message proposes an abstraction where a concrete, reviewable change belongs. "
    "He cannot approve \"a backend factory\". He can approve \"backend/Utils/"
    "UserFactory.php — a new file that centralizes all user creation\": the path, the "
    "name, the purpose. This covers a shortlist of directions — \"make it fail loud\", "
    "\"bound the memory\" — answering a request for the exact changes, which costs him "
    "a round-trip to turn into something reviewable."
)

BURIED_DECISIONS = (
    "### Name prose that buries the decisions\n"
    "Decisions and information sit in undifferentiated prose instead of headings, "
    "lists, tables, a diagram, or a file tree that a human parses fast."
)

THINKING_LOG = (
    "### Name a reply that reads as the agent's own thinking\n"
    "The message reads as an exploration log rather than a response tuned to his time. "
    "Reasoning belongs in thinking, not in the reply."
)

REPETITION = (
    "### Name the repetition or the padding\n"
    "The message repeats itself, restates an answer he already responded to, or pads "
    "with a recap of settled context or history he said he does not care about. Length "
    "alone is never the failure: a thorough proposal or a complete answer to exactly "
    "what he asked carries decision-relevant content the whole way down, and his own "
    "message being terse does not make a full, on-point reply too long."
)

FILLER_OPTION = (
    "### Name an option that exists only to be a second option\n"
    "One option in an options block is filler: it has no honest pro, or it is worse on "
    "every axis. Name that option. A genuine alternative with a real, distinct con and "
    "its own confidence is not filler, even at lower confidence — distinct-confidence "
    "options are how a proposal surfaces a real decision for him to pick. An option "
    "whose con names what the system still carries after it ships is a real "
    "alternative: debt, a second way to do one thing, a coupling, a lost capability, "
    "something a person keeps in sync by hand.\n"
    "A con that names the work instead of the result states nothing that outlives the "
    "change. Files touched, lines, modules, time. That option carries no con and pads "
    "the fork.\n"
    "A con that names a requirement the option breaks is not a choice. The agent "
    "disqualified it and still asks him to pick. Name the option and quote the line in "
    "its own con that condemns it. The requirement has to be one he stated or one the "
    "system already holds, and the con has to say it is broken. An option that is "
    "merely weaker on the axis being decided still holds every requirement: slower, "
    "cheaper, less thorough, less representative, lower confidence. That is the "
    "tradeoff he is picking between, so allow it."
)

SHRUNK_SCOPE = (
    "### Name approved work the agent shrank on its own\n"
    "He handed over approved work, and the agent changed, narrowed, broke down, "
    "deferred, fragmented, or reordered that scope on its own initiative: an invented "
    "\"for now\", an \"as a follow-up\", a \"should I do A or B first\", or the work "
    "split across turns by the agent. When he himself set the scope as a subset — "
    "named files or an area, said \"just X\", or warned the rest of the tree is his own "
    "work — doing that subset and reporting what was left out honors the scope, and a "
    "scoped commit that excludes unrelated in-flight changes and says so is hygiene. "
    "The failure is the agent shrinking his work, never the agent holding his boundary."
)

POINTER_NOT_CONTENT = (
    "### Name a pointer standing where the content belongs\n"
    "The message makes him remember something: it points at an earlier reply, an "
    "earlier decision, or code without explaining it in plain terms; states a "
    "conclusion whose basis it never gives; or states something with the context left "
    "out. The commonest form is the pointer itself — \"as I said above\", \"the "
    "proposal two messages back stands\", \"see my last reply\", \"that answer still "
    "holds\" — and the bare label put where the content belongs: a verdict opening "
    "\"3 — no, not as written\", \"B2 is wrong\", \"item 4 changes\", \"slice two "
    "instead\", where the number or code is never restated as what it stands for. He "
    "reads this message and nothing else, so a label he can only resolve in an earlier "
    "message delivers nothing. Naming a file, a skill, a command, or a document he "
    "already knows is not this."
)

UNVERIFIED_CLAIM = (
    "### Name an unverified claim about his own system\n"
    "The message states a fact about his stack, code, data, or tools that the agent "
    "never verified and that he can disprove by knowing his own project, as \"Action "
    "Scheduler is bundled inside WooCommerce\" is where there is no WooCommerce. He "
    "acts on it, so a confident wrong fact costs more than silence. Name the claim. A "
    "claim the message grounds — citing the file, the value, the config it read — "
    "passes, and so does a code or config excerpt shown to illustrate a mechanism, "
    "which he judges himself. This is about his existing system. The agent reporting "
    "its own actions this turn, including the files an approved task required, is a "
    "completion report and never this."
)

PUNTED_QUESTION = (
    "### Name a question the code already answers\n"
    "The message hands him a question, fork, or choice the agent could have resolved "
    "by reading the code, or asks him for knowledge the developer produces. The tells "
    "are punting phrases — \"that's your call\", \"you have to decide the shape\", \"do "
    "you want me to X or Y\" — on something the agent should have settled. A genuine "
    "decision the code cannot settle, presented as resolved options with distinct "
    "confidences, is the opposite of this and passes."
)

GOAL_CONTRADICTED = (
    "### Name a recommendation that fights its own goal\n"
    "A recommendation or conclusion has a stated effect that runs against the goal it "
    "serves, as restoring update checks does where the checks are what cost the "
    "performance. Name the contradiction."
)

DECORATIVE_CODE = (
    "### Name code that carries no decision\n"
    "The message makes him read code that serves no purpose for the decision in front "
    "of him: implementation pasted with no reason he needs it, or function bodies and "
    "line numbers inside an architecture-level decision. Remove the code and ask "
    "whether he can still decide. If he can, it was decoration. Code with a purpose "
    "always passes: a snippet showing a mechanism, a prose or documentation diff, a "
    "before-and-after that makes a subtle change concrete, an answer to a question "
    "about code. This never reduces the code he gets; it keeps it to code he needed."
)

UNANSWERED_QUESTION = (
    "### Name a question of his the reply leaves unanswered\n"
    "He asked a question and the reply does not answer it. His question is in his "
    "message above, in his own words: read it there, and never infer a different one. "
    "Answered means the answer is in this reply. Only a reply with no answer at all "
    "fails — one that acknowledges, promises to look into it, or proposes work in "
    "place of answering. A direct answer, a partial answer, an answer that disagrees "
    "with him, and an answer followed by a proposal are all answered."
)

USABLE_MESSAGE = (
    "### Allow a message he can use as it stands\n"
    "The message is self-contained, concrete in real paths and names and purposes, "
    "structured for fast parsing, faithful to his scope, and explains what it "
    "references inline in plain architectural terms. A short, clear, self-contained "
    "answer to a direct question is this. So is a pause to await subagents or "
    "background work the agent dispatched. So is a genuine decision resolved as far as "
    "the code allows and put to him to choose."
)


PERMISSION_ON_APPROVED_WORK = (
    "### Name a request for permission on work he already approved\n"
    "The agent asks permission to continue work that is already approved: \"shall I "
    "proceed?\", \"want me to continue?\", \"ready to move?\", \"where should I "
    "start?\" after a plan was approved or instructions were given. It should "
    "execute, not ask. A genuine architectural escalation passes — a destructive "
    "operation, a credential it does not have, a scope-changing decision with real "
    "tradeoffs — but a question with only one reasonable answer is hand-holding."
)

DEFERRED_IN_SCOPE_WORK = (
    "### Name in-scope work the agent deferred\n"
    "The agent defers work that was part of the task: \"as a follow-up\", \"in a "
    "future PR\", \"separate concern\", \"TODO\", \"out of scope\" for work that is "
    "in scope by the plan or his instructions. Genuinely unrelated work passes, and "
    "so does an action the agent physically cannot perform — DNS changes, dashboard "
    "access, server SSH, credential rotation, starting services in another "
    "environment. \"You'll need to\" is legitimate only when the agent has no way to "
    "do it itself."
)

UNFINISHED_PLAN = (
    "### Name the plan step the agent left undone\n"
    "The agent finished some plan steps and not others. Check the plan above against "
    "what the message claims. Naming the remaining items and why it stopped on a "
    "genuine blocker passes; \"this is a good stopping point\" does not."
)

CONTEXT_PRESSURE = (
    "### Name context pressure used as the reason to stop\n"
    "The agent stops mid-task citing the context window, message length, or keeping "
    "context \"manageable\". It continues until the work is done or a genuine blocker "
    "lands."
)

KNOWINGLY_WRONG_DELIVERABLE = (
    "### Name a deliverable shipped knowingly wrong\n"
    "The agent acknowledges its own deliverable is off-brief, sub-quality, or wrong, "
    "ships it, and promises a next-turn redo: \"will be regenerated next turn\", "
    "\"misses the brief and will be fixed\", \"for now here's X, I'll redo properly "
    "later\", \"next iteration\", \"TODO: redo\". No exception — recognizing the work "
    "is bad obliges the redo before stopping."
)

STRANDED_DELIVERABLE = (
    "### Name a deliverable stranded in an earlier response\n"
    "The agent sent more than one response this turn and the final message drops the "
    "reply an earlier one already gave. He acts on the last message, so a deliverable "
    "left in an earlier response is undelivered. Compare the turn content above "
    "against the last message: the substantive deliverable living only in an earlier "
    "response, or the last message pointing at it with \"above\", \"earlier\", or "
    "\"the list below\" while that content is not inside it, is this."
)

CON_HE_MUST_ACCEPT = (
    "### Name a con framed as something he must accept\n"
    "The agent frames a downside as something he should accept, absorb, or live with, "
    "rather than as a problem the option attacks or work the option still owes. The "
    "tells sit inside an options block or a recommendation: \"accept the\", "
    "\"accepting this\", \"live with\", \"the price we pay\", \"tradeoff we absorb\", "
    "\"you'll need to accept\", \"this trades X for Y\" used to ask him to swallow Y. "
    "A con is a problem to solve, and AI cost makes solving it cheap. Name the con. "
    "The word as content passes — \"you accepted X earlier\", \"the API accepts "
    "JSON\", reporting a tradeoff he already took."
)

PROPOSAL_FAILURE = (
    "### Name the proposal failure\n"
    "Judge the message against the seven named proposal failures the /propose skill "
    "defines.\n"
    "- vacuous-proposal: proposal shape — headings, slices, choice blocks — carrying "
    "no architectural decision. The brief restated in proposal layout; steps shaped "
    "as 'investigate', 'consider', 'evaluate' with no concrete change; choice blocks "
    "with two unspecified directions.\n"
    "- capability-loss: a removal that does not name what it removed, or where the "
    "protected capability now lives.\n"
    "- worse-option-shipped: the agent ships an option it identifies as suboptimal. "
    "'Going with A. B would be cleaner but…', 'A is simpler though B is more "
    "correct', any footnote pointing at a better option than the one shipped.\n"
    "- requirement-drop: a requirement he stated is missing, narrowed, deferred, or "
    "relaxed to fit the chosen path.\n"
    "- contradiction-elision: two requirements conflict, or a requirement contradicts "
    "the code, and the proposal picks a side instead of surfacing the conflict as his "
    "decision.\n"
    "- mixed-layer-pcc: the proposal asks him to decide several things where one "
    "answer obliterates the others. Surface only the gate.\n"
    "- hedged-proposal: 'likely', 'may', 'should' in the expected-behavior sense, "
    "'probably', 'might', 'could', 'perhaps', 'I would expect', 'it appears that'. A "
    "hedge is a confession the source was not read. A stated unknown is the same "
    "failure in a franker voice: the check was reachable and was not run."
)

UNREAD_CODE_CLAIM = (
    "### Name a code claim the agent never read\n"
    "The message asserts what code does, returns, calls, contains, or causes using "
    "hedges that signal the source was not read: \"likely\", \"probably\", \"should\" "
    "in the expected-behavior sense, \"may\", \"might\", \"could\", \"appears\", "
    "\"seems\", \"I would expect\". It also covers a check named instead of run — "
    "\"what I'd verify\", \"I'd check\", \"you'd want to confirm\", \"the most likely "
    "culprit\", \"the probable cause\". Quote the hedge or the named-but-unrun check. "
    "A stated gap is not an exemption: a source named as unread is this failure."
)

BASELINE_BLAMED = (
    "### Name a failure blamed on the baseline\n"
    "The agent attributes a failing test, error, or broken behavior to pre-existing "
    "state and stops or excuses itself on that basis: \"pre-existing failure\", "
    "\"already broken before\", \"already failing\", \"not caused by my change\", "
    "\"unrelated to my change\". The repo does not sit perpetually broken, and the "
    "agent's own change owns what is broken now. Name the failure it disowned. \"I "
    "have not yet determined what broke this\" is this failure too, not an honest "
    "form. A pre-existing claim proven in the message, with the failure shown running "
    "through code the change never touches, is evidence rather than deflection."
)

EDITED_BLIND = (
    "### Name a file edited without reading it\n"
    "The agent changed a file this turn without grounding the change in the code. The "
    "edited-file facts above give, per file, how much of it the agent read this "
    "session and how many of its callers it read. This fires when an edited file was "
    "barely read, or no read was recorded at all, or none of its caller files were "
    "read — the agent changed code without reading either the code or the call sites "
    "that depend on it. A fully-read file with no callers in the graph is grounded, "
    "and a trivial self-contained change to a file the agent fully read is fine."
)

UNOPENED_EDIT_PROPOSED = (
    "### Name a concrete edit proposed to a file never opened\n"
    "The message commits to editing, rewriting, replacing, or changing a specific file "
    "it never opened this session. The opened-files list above is every file the agent "
    "read; a file absent from it is unopened, unless it appears in the list of paths "
    "that do not exist, which are files the message proposes creating. Naming a file "
    "to read or investigate next is not this."
)

INVENTED_STRUCTURE = (
    "### Name structure invented with no precedent read\n"
    "The message builds or proposes a new file, skill, module, layout, or pattern, and "
    "the opened-files list shows it read nothing that could have informed it — no "
    "sibling, no similar or related example anywhere in the repo. Reading a similar "
    "file counts as precedent, so this fires only on structure invented with no "
    "relevant reading at all. Structure the agent states has genuinely no precedent, "
    "having looked, is a real architecture decision and passes."
)

PATH_INSTEAD_OF_CONTENT = (
    "### Name a file path handed over in place of the content\n"
    "The message hands him a path as the thing to read for the decision: \"the full "
    "report is at docs/…\", \"I wrote the proposal to /tmp/…\", \"see the plan file\". "
    "He decides from the message itself and never opens agent-written files to decide. "
    "Name the path. An evidence or report path cited beside an in-message deliverable "
    "is a record and passes, as does a consumption-optimized review he asked for — a "
    "published page, a diagram — named as the deliverable."
)

FORWARDED_RECOMMENDATION = (
    "### Name a subagent's recommendation forwarded as the decision\n"
    "The message hands him a subagent's ranking or verdict instead of the agent's own. "
    "A subagent saw one slice; the agent holds the project, so the agent ranks and says "
    "so in its own voice. The matched wording is in the facts above: quote it. A "
    "subagent's finding cited as evidence passes, and so does the agent agreeing with "
    "one after weighing it itself."
)

UNCHECKED_CLAIM = (
    "### Name a claim handed over unchecked\n"
    "The message reports that the agent did not check something, instead of checking it. "
    "The matched wording is in the facts above: quote it. He acts on this reply, so a gap "
    "named in it is work he now has to do himself. It passes only when the check needs "
    "his credentials, his production system, or his own eyes, and the message asks him "
    "for that one action rather than filing it as a finding. It also passes when the "
    "sentence is about his own undecided call rather than the agent's unchecked claim."
)

FINISHED_TURN = (
    "### Allow a turn that is genuinely finished\n"
    "The work is complete with nothing deferred, or the message summarizes work "
    "completed across the conversation, which is accurate reporting and never an "
    "over-claim. A genuine architectural question with real tradeoffs, a destructive "
    "operation put to him, or a request for credentials the agent does not have is "
    "this. So is analysis or options he asked for as the deliverable, work correctly "
    "scoped out as unrelated, and an answer to his question. A turn that really did "
    "dispatch a subagent or background work never reaches you — the code checks the "
    "turn's own records and returns before this call — so a message that merely says "
    "it is holding or waiting, with no dispatch behind it, is a stop to judge like "
    "any other."
)


def _rules(intent, current_state, mode, can_write=True,
           has_opened=False, has_edit_facts=False,
           forwarded=False, unsettled=False):
    """The Rules this turn can produce, in order.

    Each Rule pairs with the turn fact that admits it; None admits it always. A
    Rule the turn cannot produce is left out of the call rather than carried with
    a Condition the judge has to apply to itself.

    Three groups carry a fact rather than always applying. The approved-work group
    needs him to have handed work over: the state axis reads `execute` because he
    typed /execute, and the turn is new work rather than a question or a
    correction. The reading group needs the read log to be in the prompt — without
    it, "not in the opened list" is true of every file in the repo, and a measured
    control run showed those Rules blocking 3 of 5 sound proposals on exactly that.
    The rest turn on the intent they judge."""
    def approved_work(i, s, m):
        return s == "execute" and i == "action"

    catalog = (
        (INVENTED_TERM, None),
        (COLLIDING_TERMS, None),
        (TWO_READINGS, None),
        (CATEGORY_NOT_CHANGE, None),
        (BURIED_DECISIONS, None),
        (THINKING_LOG, None),
        (REPETITION, None),
        (FILLER_OPTION, None),
        (STRANDED_DELIVERABLE, None),
        (POINTER_NOT_CONTENT, None),
        (UNVERIFIED_CLAIM, None),
        (UNREAD_CODE_CLAIM, None),
        (BASELINE_BLAMED, None),
        (GOAL_CONTRADICTED, None),
        (DECORATIVE_CODE, None),
        (PATH_INSTEAD_OF_CONTENT, None),
        (CON_HE_MUST_ACCEPT, None),
        (KNOWINGLY_WRONG_DELIVERABLE, None),
        # Shrinking scope, asking permission, deferring and stopping short are
        # failures only while implementing work he approved. A question or a
        # correction is an alignment turn whatever the state says, and surfacing a
        # decision there is the job.
        (SHRUNK_SCOPE, approved_work),
        (PERMISSION_ON_APPROVED_WORK, approved_work),
        (DEFERRED_IN_SCOPE_WORK, approved_work),
        (UNFINISHED_PLAN, approved_work),
        (CONTEXT_PRESSURE, approved_work),
        # The mirror of that boundary: punting and proposal quality are judged on
        # the turns where the proposal was the deliverable.
        (PUNTED_QUESTION, lambda i, s, m: s == "propose" or i in ("question", "correction")),
        (PROPOSAL_FAILURE, lambda i, s, m: s == "propose"),
        (UNANSWERED_QUESTION, lambda i, s, m: i == "question"),
        # Admitted by the facts they read, never by the kind of turn.
        (FORWARDED_RECOMMENDATION, lambda i, s, m: forwarded),
        (UNCHECKED_CLAIM, lambda i, s, m: unsettled),
        (EDITED_BLIND, lambda i, s, m: has_edit_facts),
        (UNOPENED_EDIT_PROPOSED, lambda i, s, m: has_opened),
        (INVENTED_STRUCTURE, lambda i, s, m: has_opened),
        (USABLE_MESSAGE, None),
        (FINISHED_TURN, lambda i, s, m: can_write),
    )
    return [text for text, admits in catalog
            if admits is None or admits(intent, current_state, mode)]


def _turn_facts(intent, current_state, mode, can_write,
                permission="", forwarded="", unsettled=""):
    """What this turn is, read off the spine rather than re-derived from the text.

    Every line is a fact, never an instruction: a judge that faults the agent for
    not editing or not running a check writes that into `reason`, and `reason` is
    what the agent acts on next. The approval line is the one it cannot infer —
    nothing in his words says "approved", and the only record of it is the state
    axis he moved himself.

    A matched phrase arrives with the matched wording, never as a bare category. A
    category makes the judge re-find what the matcher already found, and it can
    then rule on words that never matched."""
    facts = "- The architect's intent this turn: %s.\n- Mode: %s. State: %s.\n" % (
        intent, mode, current_state)
    if not can_write:
        facts += (
            "- The agent cannot write files or change the tree in this mode and state. "
            "Nothing it left unedited, unapplied, or unfinished is a failure of this "
            "turn.\n"
            "- Nothing is approved this turn either. Approval is the architect typing "
            "/execute, which moves the state axis. While it reads 'propose', the agent "
            "putting its proposal to him and waiting IS the deliverable.\n"
        )
    if permission and can_write:
        facts += "- The message contains a permission-seeking phrase: \"%s\".\n" % permission
    if forwarded:
        facts += ("- The message contains forwarded-recommendation wording: \"%s\".\n"
                  % forwarded)
    if unsettled:
        facts += "- The message contains unchecked-claim wording: \"%s\".\n" % unsettled
    return facts


def _eval_prompt(request, last_msg, intent, current_state, mode, can_write,
                 plan="", turn_evidence="", read_facts="", permission="",
                 forwarded="", unsettled="", has_opened=False, has_edit_facts=False):
    plan_block = "The plan for this session:\n%s\n---\n" % plan if plan else ""
    turn_block = ""
    if turn_evidence:
        turn_block = (
            "This turn's responses in full, thinking as size markers, and every tool "
            "call with its real outcome, chronological — so a deliverable stranded in "
            "an earlier response and a failed edit are both visible:\n%s\n---\n"
        ) % turn_evidence
    return (
        "Judge the agent's last message to the architect.\n\n"
        "%s\n"
        "%s"
        "The architect's last message, which this reply answers:\n%s\n---\n"
        "The agent's LAST MESSAGE, which he reads and acts on:\n%s\n---\n"
        "%s%s"
        "The Rules for this turn. Each names one failure, and the last ones name the "
        "message and the turn that pass:\n\n%s\n\n"
        "Return JSON. Allow: {\"allow\": true}. Block: {\"allow\": false, \"reason\": "
        "\"the Rule's title, then the offending part\"}."
    ) % (_turn_facts(intent, current_state, mode, can_write,
                     permission, forwarded, unsettled),
         plan_block, request, last_msg, turn_block, read_facts,
         "\n\n".join(_rules(intent, current_state, mode, can_write,
                            has_opened, has_edit_facts,
                            bool(forwarded), bool(unsettled))))


if __name__ == "__main__":
    sys.exit(main())
