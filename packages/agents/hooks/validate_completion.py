#!/usr/bin/env python3
"""Validate agent work before allowing stop (Stop hook).

Gates, in order:
  1. Fast combo — ExitPlanMode in the current turn + a permission-seeking phrase
  2. Deterministic forwarded-recommendation phrase in the markdown-stripped message
  3. LLM validation — runs on every stop turn (no trigger fast-path; the
     architect's rule: a short proposal must not skip it)

The judge is mode-aware off the spine's `state`: the completion and grounded-edit
checks fire only while EXECUTING approved work. While proposing or answering, the
architect is seeking alignment, so the judge never pushes toward edits or
"keep going" — it only guards honesty and proposal quality.

Any infrastructure failure exits 0 (allows the stop) — the hook never blocks on its
own brokenness. Exit 2 blocks the stop; exit 0 allows it.
"""

import json
import os
import re
import shutil
import subprocess
import sys

from lib import feedback, transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_state import bump_gate_block, gate_block_count, load_state

# A judgment-based block fires at most this many times per turn, then the gate
# yields for the rest of the turn — it makes its point once and trusts the agent.
GATE_BLOCK_CAP = 1

# Below this read-fraction, an edited file counts as barely-read for the
# blind-edit gate. The tracer records an edit's PreToolUse shoulder as a
# whole-file read, so an edited file's coverage is usually near 1.0 (see the
# coverage note in the read-log section below); the floor catches the residual
# partial-read-then-edit case and a file that never recorded any read at all.
COVERAGE_FLOOR = 0.5

# Claude only, for the reason babysitter.py carries: every input this validator
# reads — the turn's boundary, its edits, its tool outcomes — is parsed out of
# Claude's transcript, and codex's rollout has none of those record shapes. On
# codex it would report zero mutations for every turn, always.
BINDING = {
    "events": {"Stop": []},
    "timeout": 90,
    "harness": "claude",
}

JSON_SCHEMA = '{"type":"object","properties":{"allow":{"type":"boolean"},"reason":{"type":"string"}},"required":["allow"]}'
SYSTEM_PROMPT = ("You are a completion validator with LIMITED context — the agent's final "
                 "message and a little session data. You are not the orchestrator. Judge "
                 "whether the message is reasonable and usable ON ITS FACE, never whether it "
                 "obeys instructions you only half-see; you do not hold the architect's full "
                 "intent, so never block a sound message for differing from what you imagine "
                 "was asked, and assume the architect has common sense. You see only a slice of "
                 "the session — you do not know what earlier turns finished, whether a "
                 "background command completed, or whether a listed requirement is still open — "
                 "so NEVER invent that state to block, and never force the agent to resurface "
                 "already-finished work: a message that omits completed work is not dropping it. "
                 "Block ONLY when the work is genuinely incomplete or the message actively "
                 "misleads — the test is whether, without your block, he gets a response he must "
                 "send back for a rewrite. When unsure, or when judging would need context you "
                 "lack (what was edited, repo state, prior decisions), ALLOW: a wrong block "
                 "costs him a whole turn and real money; a clear-but-imperfect message costs "
                 "nothing. Never block to trim a word, tighten a phrase, add a citation, or make "
                 "a usable reply marginally better. Cumulative reporting of the whole "
                 "conversation's work is accurate, not a false claim. Zero edits or tool calls "
                 "THIS TURN never makes a completion report false or incomplete — work finished "
                 "in an earlier turn correctly shows none now, and absence of this-turn evidence "
                 "is not grounds to block. Block a completion claim on only two things: tool "
                 "evidence shown this turn that CONTRADICTS it (a tests-pass claim beside a FAILED "
                 "run), or the message ITSELF showing the work is unfinished — it defers ('I will "
                 "continue when...'), names remaining steps, points at a file instead of giving "
                 "the asked deliverable, or reports a change a boundary forbids. Never read 'not "
                 "evidenced this turn' as 'incomplete.' The architect reads "
                 "architecture — file purpose, data ownership, the public API, exact schema "
                 "changes — and prose and doc diffs, never line-by-line code unless he asked for "
                 "code this turn; naming the files, API, and cited values IS evidence the agent "
                 "read the source, so never demand a re-scan, inlined code, line numbers, or "
                 "re-delivery of an answer it already gave and he already responded to. You "
                 "only RAISE the concern in `reason`; you never prescribe a fix, an edit, or a "
                 "next step — the main agent has the full context and owns that. Output "
                 "structured JSON only.")

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

FORWARDED_BLOCK_MSG = """Potential issue: the message forwards a subagent's recommendation ("%s") as the decision, rather than the agent ranking it in its own voice. A subagent saw a slice; the agent holds the project."""


def _strip_markdown(text):
    """Strip markdown quote constructs so the forwarded-recommendation gate
    doesn't false-fire on test output, doc snippets, examples, or user-quote
    echoes. Fenced blocks (line-structural) and
    blockquote/indented lines are dropped whole; inline code and quoted spans
    are stripped within surviving lines. Bold and italic stay intact — those
    mark the agent's own assertions, not quotes."""
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
    # awk's print adds a trailing newline per surviving line; join matches that
    # for substring matching (the only consumer lowercases and greps).
    return "\n".join(out)


# --- Read-log facts for the blind-edit gates --------------------------------
#
# The tracer's session log records, per file, how much of it the agent read this
# session — surfaced by `trace docs status`. A read counts toward coverage no
# matter how it happened: the builtin Read/Edit tools, a `trace read`/`context`
# call, and a shell `cat` all flow through the same `trace context` shoulder
# (enrich_on_read.py) into the same accumulator. The caller graph (`trace info`
# top_callers) names the files that depend on an edited file. The two new rules
# below reason over these facts; the deterministic code only gathers them.


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
    gates need: (coverage, opened). coverage maps a read file's realpath ->
    read_fraction; opened is the set of realpaths the agent actually read this
    session (kind read_file — doc-injected and directory entries are not "opened
    to edit"). Empty pair on any failure, so the gates degrade to silent."""
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
    that rebuilds the graph (architecture::get) and persists it, so a following
    `trace info` in a fresh process validates that entry and serves real callers.
    Run once for its side effect before any caller lookup; output discarded."""
    try:
        subprocess.run(["trace", "status", "--json"],
                       cwd=cwd, env=env, capture_output=True, timeout=20)
    except Exception:
        pass


def _caller_files(file_path, cwd, env):
    """Repo-relative source files that directly call the edited file, via
    `trace info <file>` top_callers (up to ten direct callers). Reads the graph
    load-only, so the caller must `_warm_graph` first against a dirty tree. Empty
    on any failure — the gate then sees a file with no known callers."""
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
    """Per-edited-file fact lines plus whether any edit is a blind-edit risk. The
    lines feed the judge (rule 11); the bool routes an otherwise sub-threshold
    stop to the judge so a single blind edit cannot slip past on the fast path."""
    _warm_graph(cwd, env)
    lines, risk = [], False
    for f in edited_files:
        fraction = coverage.get(os.path.realpath(f))
        line, file_risk = _edit_fact_line(f, fraction, _caller_files(f, cwd, env), opened)
        lines.append(line)
        risk = risk or file_risk
    return "\n".join(lines), risk


def _edit_facts_context(edited_facts_block, opened):
    """The judge-facing facts section for rules 11 and 12: the per-edited-file
    coverage/caller lines, then the opened-files list. Empty string when there is
    nothing to say (no edits this turn and nothing read), so the prompt is
    unchanged on turns the gates don't apply to."""
    parts = ""
    if edited_facts_block:
        parts += (
            "Files this turn EDITED, with read-coverage and caller-read facts "
            "(for rule 11):\n%s\n---\n" % edited_facts_block
        )
    if opened:
        listing = "\n".join(sorted(opened))
        parts += (
            "Files the agent OPENED (read) this session — every other file is "
            "unopened (for rule 12):\n%s\n---\n" % listing
        )
    return parts


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0

    transcript_path = field(event, "transcript_path", "")
    last_msg = field(event, "last_assistant_message", "")

    state = load_state(session_id)
    current_state = state.get("state") or "proposing"
    commit_requested = bool(state.get("commit_requested", False))
    executing = current_state == "executing"

    last_msg_lower = last_msg.lower()
    last_msg_stripped_lower = _strip_markdown(last_msg).lower()

    has_phrase = any(p in last_msg_lower for p in PERMISSION_PHRASES)

    # Current turn — raw lines for the substring gates, parsed records for evidence
    turn_lines = transcript.current_turn_lines(transcript_path)
    recs = transcript.records(transcript_path)
    turn = transcript.current_turn(recs)
    has_recent_plan_exit = any("ExitPlanMode" in line for line in turn_lines)

    # Layer 1: ExitPlanMode in current turn + permission-seeking phrase
    if has_phrase and has_recent_plan_exit:
        return feedback.raise_concern("validate_completion", "Stop",
                              "Potential issue: the plan is approved, but the message asks permission to start the approved work.")

    # Deterministic gate: forwarded-recommendation phrase in stripped message
    forwarded = next((p for p in FORWARDED_PHRASES if p in last_msg_stripped_lower), "")
    if forwarded:
        return feedback.raise_concern("validate_completion", "Stop", FORWARDED_BLOCK_MSG % forwarded)

    # The judge already blocked this turn — yield rather than re-block the same
    # stop on a judgment call, which is what freezes the session. The precise
    # deterministic gates above still fire every turn; only this one is capped.
    if gate_block_count(session_id, "validate_completion") >= GATE_BLOCK_CAP:
        return 0

    # Stopping to await dispatched work is an async pause, not an unfinished stop —
    # the agent resumes when the dispatch wakes it. Never gate that as incomplete.
    if transcript.awaits_async_work(turn_lines):
        return 0

    # Interview state turns the LLM judge off for speed; the cheap deterministic
    # gates above still run.
    if current_state == "interview":
        return 0

    # The judge runs on every stop turn — no trigger fast-path. Mutation count
    # still feeds the prompt; the blind-edit facts (rules 11/12) are gathered every
    # turn the tracer is present, so the opened set and per-edit coverage always
    # reach the judge.
    mutations = sum(
        1 for line in turn_lines
        if '"name":"Edit"' in line or '"name":"Write"' in line or '"name":"MultiEdit"' in line
    )

    edited_files = transcript.edited_paths(turn)
    coverage, opened, edited_facts_block = {}, set(), ""
    if shutil.which("trace"):
        cwd = field(event, "cwd", "") or os.getcwd()
        env = _trace_env(session_id)
        coverage, opened = _read_log_status(cwd, env)
        if edited_files:
            edited_facts_block, _ = _edited_facts(
                edited_files, coverage, opened, cwd, env)

    # Commit requested → allow stop (wrapping up)
    if commit_requested:
        return 0

    # Layer 2: LLM validation
    plan_content = transcript.clamp(transcript.plan_content(recs))
    recent_user_msgs = transcript.clamp(transcript.recent_user_texts(recs, 4))
    current_turn_content = transcript.clamp(transcript.turn_evidence(turn))

    plan_context = ""
    if plan_content:
        plan_context = "Plan for this session:\n%s\n---\n" % plan_content

    session_context = (
        "Session state: state=%s, commit_requested=%s\n"
        "ExitPlanMode in current turn: %s\n"
        "Permission-seeking phrase detected: %s\n"
        "File mutations this session: %s\n"
        "---\n"
    ) % (
        current_state, _shell_bool(commit_requested),
        _shell_bool(has_recent_plan_exit), _shell_bool(has_phrase), mutations,
    )

    current_turn_context = ""
    if current_turn_content:
        current_turn_context = (
            "This turn's responses in full, thinking as size markers, and each tool "
            "call with its real outcome (chronological — to check whether an earlier "
            "response holds a deliverable the final message dropped, and whether any "
            "edits failed):\n%s\n---\n"
        ) % current_turn_content

    edit_facts_context = _edit_facts_context(edited_facts_block, opened)

    eval_prompt = _eval_prompt(
        plan_context, session_context, recent_user_msgs, last_msg,
        current_turn_context, edit_facts_context, current_state,
    )

    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=eval_prompt,
                       schema=JSON_SCHEMA, session_id=session_id, hook="validate_completion")
    if not result:
        return 0

    allow = result.get("allow", True)
    if allow is False:
        reason = result.get("reason") or "the work may be incomplete"
        bump_gate_block(session_id, "validate_completion")
        return feedback.raise_concern("validate_completion", "Stop", "Potential issue: %s" % reason)

    return 0


def _shell_bool(value):
    return "true" if value else "false"


def _eval_prompt(plan_context, session_context, recent_user_msgs, last_msg,
                 current_turn_context, edit_facts_context, current_state="proposing"):
    mode_header = (
        "Classify THIS turn as ALIGNMENT or EXECUTION from the recent user messages "
        "above; the session state ('%s') is only a hint, not the answer — an "
        "'executing' state can persist from an earlier task while this turn is the "
        "architect asking or correcting.\n"
        "- ALIGNMENT turn: the architect is asking a question, correcting or refining "
        "your previous output, or you are proposing/shaping a design. Apply ONLY rules "
        "5, 6, 7, 8, 9, and 10. Do NOT apply rules 1, 2, 3, 11, 12, or 13. A clean stop "
        "to answer, align, propose, ask a real question, or present findings is CORRECT "
        "— even when state is 'executing'. Never push the agent to edit, finish, stop "
        "asking, open a file before proposing, or continue.\n"
        "- EXECUTION turn: the architect handed over approved work and the agent is "
        "implementing it. Every rule below applies; hold it to finishing what was "
        "approved.\n"
        "When unsure which, treat it as ALIGNMENT — a wrong push to keep editing costs "
        "the architect a whole loop.\n\n"
    ) % current_state
    mode_header = (
        "This gate protects alignment and quality. Block only to stop a real failure "
        "below — never to push the agent to edit faster, rush code, or keep moving "
        "when the architect wants to align first.\n\n" + mode_header
    )
    return (
        'Evaluate whether this agent should be allowed to stop.\n\n'
        '%s%sRecent user messages:\n'
        '%s\n'
        '---\n'
        'Agent\'s last message before stopping (this is what the user will see in the final visible text):\n'
        '%s\n'
        '---\n'
        '%s'
        '%s'
        '%s'
        'BLOCK the stop if ANY of these are true:\n\n'
        '1. PREMATURE STOP — the agent asks permission to continue work that is already approved. Signals: "shall I proceed?", "want me to continue?", "ready to move?", "where should I start?" after a plan was approved or instructions were given. The agent should execute, not ask.\n'
        '   Exception: genuine architectural escalation (destructive operation, credential needed, scope-changing decision with real tradeoffs). If the question has only one reasonable answer, it\'s not a genuine escalation — it\'s hand-holding.\n\n'
        '2. DEFERRAL OF IN-SCOPE WORK — the agent defers work that was part of the original task. Signals: "as a follow-up", "in a future PR", "separate concern", "TODO", "out of scope" for work that IS in scope based on the plan or user\'s instructions.\n'
        '   Exception: work genuinely unrelated to the current task, OR actions the agent physically cannot perform (DNS changes, dashboard access, server SSH, credential rotation, starting services in a different environment). "You\'ll need to" is legitimate ONLY when the agent has no way to do it itself.\n\n'
        '3. INCOMPLETE WORK — the agent completed some plan steps but not all. Check the plan (if provided) against what the agent claims to have done.\n'
        '   Exception: the agent explicitly identifies remaining items AND explains why it stopped (genuine blocker, not "this is a good stopping point").\n\n'
        '4. CONTEXT PRESSURE EXCUSE — the agent stops citing context window, message length, or "manageable" context as the reason, mid-task. The agent should continue executing until the work is done or a genuine blocker is hit.\n\n'
        '5. SHIPPED-AND-DEFERRED — the agent acknowledges its own deliverable is off-brief, sub-quality, or wrong, then ships it anyway and promises a next-turn redo. Signals in the agent\'s last message: "will be regenerated next turn", "misses the brief and will be fixed", "for now here\'s X, I\'ll redo properly later", "next iteration", "TODO: redo", "acknowledged on the correction — most of the list misses". No exception. If the agent recognized the work is bad, it must redo before stopping. The whole point of the rule is preventing this exact failure shape.\n\n'
        '6. BURIED DELIVERABLE — the agent sent more than one response this turn and the final message drops the full reply that an earlier response already gave. The last message is the deliverable the user acts on; a reply stranded in an earlier response of the turn does not count as delivered. Compare the full current-turn content above against the last message: if the substantive deliverable lives only in an earlier response, or the last message points at it with "above" / "earlier" / "the list below" while that content is not actually inside the last message, that\'s burial. Block.\n\n'
        '7. ACCEPT-FRAMING IN A PROPOSAL — the agent frames a con or downside as something the user should accept, absorb, or live with, rather than as a problem the option attacks or as outstanding work the option still owes. Signals inside a /pcc, proposal, options block, or recommendation in the last message: "accept the", "accepting this", "live with", "the price we pay", "tradeoff we absorb", "you\'ll need to accept", "this trades X for Y" used to ask the user to swallow Y. A con is a problem to solve; AI cost makes solving it cheap. Block so the agent either folds the solve into the option or surfaces the con as outstanding work, never as a compromise the user signs off on.\n'
        '   Exception: word-as-content is fine — "you accepted X earlier", "the API accepts JSON", reporting that the user has already taken a tradeoff. The block is on framing a fresh con as inevitable.\n\n'
        '8. PROPOSAL-FAILURE (proposing-state turns only — state=\'proposing\') — judge the agent\'s last message against the seven named proposal failures defined in the /propose skill. Block when any of the seven applies. This rule fires ONLY when state is \'proposing\'; in any other state, skip to the ALLOW gates.\n\n'
        '   - vacuous-proposal: the proposal uses proposal shape (headings, slices, choice blocks) but the content carries no architectural decision. Restating the brief in proposal layout; steps shaped like \'investigate\', \'consider\', \'evaluate\' with no concrete change; choice blocks with two unspecified directions. Block.\n'
        '   - capability-loss: a removal in the proposal does not name what it removed, or does not name where the protected capability now lives. The proposal pursues the brief but silently deletes a guard, code path, validation, or behavior. Block.\n'
        '   - worse-option-shipped: the agent shipped an option it itself identifies as suboptimal. Signals: \'Going with A. B would be cleaner but...\', \'A is simpler though B is more correct\', any footnote pointing at a better option than the one shipped. Block.\n'
        '   - requirement-drop: a requirement the user stated in the brief is missing from the proposal, narrowed, deferred (\'out of scope for this pass\'), or relaxed to fit a chosen path. Block.\n'
        '   - contradiction-elision: two requirements conflict, or a requirement contradicts what the code does, and the proposal silently picks a side instead of surfacing the conflict as a decision the architect makes. Block.\n'
        '   - mixed-layer-pcc: the proposal asks the architect to decide multiple things where one decision would obliterate the others. Three choice blocks where deciding the first one differently discards the second and third. Block — surface only the gate.\n'
        '   - hedged-proposal: the proposal uses \'likely\', \'may\', \'should\' (in the sense of expected behavior), \'probably\', \'might\', \'could\', \'perhaps\', \'I would expect\', \'it appears that\', \'this probably means\'. Hedge words are confessions that the source was not read. Block. Exception: \'I have not checked X\' is allowed and correct — the ban is on hedges substituting for reading.\n\n'
        '   For this rule, the deliverable shape (proposal headings, slices, choice blocks, /pcc-style options) is the signal that the turn is producing a proposal. Apply the seven failures to the substance of that proposal, not to incidental prose.\n\n'
        '9. UNVERIFIED CODE CLAIM (any state) — the last message asserts what code does, returns, calls, contains, or causes using hedge words that signal the source was not read: "likely", "probably", "should" (expected-behavior sense), "may", "might", "could", "appears", "seems", "I would expect", "this probably means". It ALSO fires when the agent names a check it WOULD run instead of running it and showing the result — "what I\'d verify", "what I would verify", "I\'d check", "I would check", "you\'d want to confirm", "the most likely culprit", "the probable cause" — describing the verification rather than doing it. A guess written as fact, or a promised-but-unrun check, wastes the architect a turn to correct. Block so the agent reads the source or runs the check and restates with specific facts.\n'
        '   Exception: "I have not checked X" / "I haven\'t verified Y" is honest and allowed — the block is on hedges substituting for a read and on naming a check instead of running it, not on stated uncertainty. On proposing-state turns this is already covered by rule 8; apply it here for every other state.\n\n'
        '10. BASELINE-BLAME DEFLECTION (any state) — the agent attributes a failing test, error, or broken behavior to pre-existing state rather than its own change, and stops or excuses itself on that basis. Signals in the last message: "pre-existing failure", "already broken before", "already failing", "baseline failure", "not caused by my change", "unrelated to my change", "that was already broken". The repo does not sit perpetually broken; the agent\'s own change owns whatever is broken now. Block so the agent investigates and fixes the failure instead of disowning it.\n'
        '   Exception: stating that the cause is not yet known is honest — "I have not yet determined what broke this" is allowed. A pre-existing claim PROVEN in the message — the same failing command shown green on the pre-change baseline and red after — is evidence, not deflection; allow it. Blaming the baseline without that run to justify stopping is not.\n\n'
        '11. EDITED-BLIND (any state) — the agent changed a file this turn without grounding the change in the code. The "Files this turn EDITED" facts above give, per edited file, how much of that file the agent read this session and how many of the files that call it the agent read. Block when an edited file was barely read (a low read-fraction, or no read recorded at all) OR none of its caller files were read — the agent changed code without reading either the code itself or the call sites that depend on it, so it cannot know the change is correct or that it did not break a caller. Applies to EVERY edited file, not only the heavily-depended-on ones.\n'
        '   Exception: a fully-read file with no caller files in the graph is grounded — do not block on absent callers alone. A trivial self-contained change the agent fully read (comment, copy, version string) is fine. If no edited-file facts are shown, this rule does not apply.\n\n'
        '12. PROPOSED-EDIT-UNOPENED (any state) — the agent\'s last message proposes editing, rewriting, replacing, or changing a specific file it never opened this session. The "Files the agent OPENED" list above is every file the agent actually read; a file absent from it is unopened. If the last message commits to a concrete edit of an unopened file, block — the agent is proposing to change code it has not read. Match by path or filename against the opened list.\n'
        '   Exception: naming a file as something to read, investigate, or look at next is fine — the block is on proposing a concrete edit to an unread file, not on mentioning one. If no opened-files list is shown, this rule does not apply.\n\n'
        '13. INVENTED-STRUCTURE-NO-PRECEDENT (any state) — the message builds or proposes a NEW file, skill, module, layout, or pattern, and the "Files the agent OPENED" list above shows it read nothing that could have informed it — no sibling, no similar or related example anywhere in the repo. Reading a similar or related file counts as precedent; this fires ONLY on structure invented with no relevant reading at all. Block; tell the agent to read how the repo already does this kind of thing and follow it.\n'
        '   Exception: if no opened-files list is shown, this rule does not apply. Structure the agent states has genuinely no precedent (it looked, none exists) is a real architecture decision — allow it.\n\n'
        '14. FILE-PATH-AS-DELIVERABLE (any state) — the last message hands the architect a file path as the thing to read for the decision: "the full report is at docs/…", "I wrote the proposal to /tmp/…", "see the plan file". The architect decides from the message itself; he never opens agent-written files to decide. Block so the agent carries the decision content into the message.\n'
        '   Exception: an Evidence or report path cited alongside an in-message deliverable is a record, not the deliverable — allow it. A consumption-optimized review the architect asked for (a published HTML review, a diagram) named as the deliverable is allowed.\n\n'
        'ALLOW the stop if ANY of these are true:\n'
        '- Work is genuinely complete — all plan items done, no deferred work\n'
        '- The message summarizes work completed across the conversation — cumulative reporting of earlier-turn work is accurate, never a false claim or over-claim\n'
        '- Agent is asking a genuine architectural question with multiple viable options and real tradeoffs\n'
        '- Agent is asking about a destructive operation (git force-push, DB drop, file deletion)\n'
        '- Agent needs credentials, API keys, or access it doesn\'t have\n'
        '- Agent is presenting analysis/options that were the requested deliverable (user asked for research, agent delivered research)\n'
        '- Agent correctly scoped out genuinely unrelated work found during implementation\n'
        '- Agent dispatched background subagents or tasks this turn and is pausing to await them — a legitimate async wait, not incomplete work; the agent resumes when a subagent wakes it\n'
        '- User\'s last message was a question — agent answered it and stopped\n\n'
        'Return JSON:\n'
        '- Allow: {"allow": true}\n'
        '- Block: {"allow": false, "reason": "the specific issue found"}'
    ) % (plan_context, session_context, recent_user_msgs, last_msg,
         current_turn_context, edit_facts_context, mode_header)


if __name__ == "__main__":
    sys.exit(main())
