#!/usr/bin/env python3
"""Validate agent work before allowing stop (Stop hook).

Three-stage gate, in order:
  1. Fast combo — ExitPlanMode in the current turn + a permission-seeking phrase
  2. Deterministic forwarded-recommendation phrase in the markdown-stripped message
  3. LLM validation (triggered by phrase / mutations>=3 / deliverable-shaped turn)

Phase tracking lives on the session spine, the same control state the
proposal/commit guards read. validation_phase >= 3 breaks the loop and allows. Any
infrastructure failure exits 0 (allows the stop) — the hook never blocks on its own
brokenness. Exit 2 blocks the stop; exit 0 allows it.
"""

import re
import sys

from lib import transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_state import load_state, merge_state

BINDING = {
    "events": {"Stop": []},
    "timeout": 90,
    "harness": "all",
}

JSON_SCHEMA = '{"type":"object","properties":{"allow":{"type":"boolean"},"reason":{"type":"string"},"instruction":{"type":"string"}},"required":["allow"]}'
SYSTEM_PROMPT = "You are a completion validator. Decide whether the agent should be allowed to stop or forced to continue. Output structured JSON only."

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

FORWARDED_BLOCK_MSG = """Forwarded recommendation detected: "%s"

A subagent's recommendation is one of its findings. Strip it,
re-rank the survivors with your own /pcc, and recommend one in
your own voice. The subagent saw a slice; you hold the project.
See /subagents "You do the ranking. Subagents do not."
"""


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


def _increment_phase(session_id):
    current = load_state(session_id).get("validation_phase") or 0
    merge_state(session_id, {"validation_phase": current + 1})


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
    validation_phase = state.get("validation_phase") or 0

    # Infinite loop breaker — allow stop after 3 blocks
    try:
        if int(validation_phase) >= 3:
            return 0
    except (TypeError, ValueError):
        pass

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
        _increment_phase(session_id)
        sys.stderr.write("The plan is approved. Continue executing — do not ask permission to start.\n")
        return 2

    # Deterministic gate: forwarded-recommendation phrase in stripped message
    forwarded = next((p for p in FORWARDED_PHRASES if p in last_msg_stripped_lower), "")
    if forwarded:
        _increment_phase(session_id)
        sys.stderr.write(FORWARDED_BLOCK_MSG % forwarded)
        return 2

    # Layer 2 trigger gate: phrase OR mutations >= 3 OR deliverable-shaped turn
    mutations = sum(
        1 for line in turn_lines
        if '"name":"Edit"' in line or '"name":"Write"' in line or '"name":"MultiEdit"' in line
    )

    has_deliverable_text = transcript.assistant_text_len(turn) > 1500

    # No phrase AND low mutations AND not a deliverable turn → allow stop
    if not has_phrase and mutations < 3 and not has_deliverable_text:
        return 0

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

    eval_prompt = _eval_prompt(
        plan_context, session_context, recent_user_msgs, last_msg, current_turn_context
    )

    result = run_model(SYSTEM_PROMPT, eval_prompt, JSON_SCHEMA,
                       session_id=session_id, hook="validate_completion")
    if not result:
        return 0

    allow = result.get("allow", True)
    if allow is False:
        reason = result.get("reason") or "Incomplete work detected"
        instruction = result.get("instruction") or "Review and complete remaining work"
        _increment_phase(session_id)
        sys.stderr.write("%s\n\n%s\n" % (reason, instruction))
        return 2

    return 0


def _shell_bool(value):
    return "true" if value else "false"


def _eval_prompt(plan_context, session_context, recent_user_msgs, last_msg, current_turn_context):
    return (
        'Evaluate whether this agent should be allowed to stop.\n\n'
        '%s%sRecent user messages:\n'
        '%s\n'
        '---\n'
        'Agent\'s last message before stopping (this is what the user will see in the final visible text):\n'
        '%s\n'
        '---\n'
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
        'ALLOW the stop if ANY of these are true:\n'
        '- Work is genuinely complete — all plan items done, no deferred work\n'
        '- Agent is asking a genuine architectural question with multiple viable options and real tradeoffs\n'
        '- Agent is asking about a destructive operation (git force-push, DB drop, file deletion)\n'
        '- Agent needs credentials, API keys, or access it doesn\'t have\n'
        '- Agent is presenting analysis/options that were the requested deliverable (user asked for research, agent delivered research)\n'
        '- Agent correctly scoped out genuinely unrelated work found during implementation\n'
        '- User\'s last message was a question — agent answered it and stopped\n\n'
        'Return JSON:\n'
        '- Allow: {"allow": true}\n'
        '- Block: {"allow": false, "reason": "specific issue found", "instruction": "what the agent should do next"}'
    ) % (plan_context, session_context, recent_user_msgs, last_msg, current_turn_context)


if __name__ == "__main__":
    sys.exit(main())
