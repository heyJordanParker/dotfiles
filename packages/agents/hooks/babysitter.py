#!/usr/bin/env python3
"""Block a reply that is not optimized for the architect to read and act on (Stop).

Runs on every stop turn alongside validate_completion.py, but judges a different
axis. validate_completion owns work-integrity — is the work done, is it grounded
in the code it changed. babysitter owns the LAST MESSAGE itself — is it worth the
architect's time: self-contained, concrete, structured, clearly worded, scope-
faithful, free of anything the architect must remember. It never judges whether
files were read; that is validate_completion's deterministic job, and guessing it
here is what made the two gates duplicate.

It reads only the last turn — the architect's last message and the agent's reply —
because correctness of the reply is judged from the reply, not the session.

One run_model call. allow=false blocks the stop (exit 2) with the offending rule
and a fix; allow=true lets it pass. Any infrastructure failure (recursion guard,
model unavailable, parse error) returns 0 and never blocks — a broken model must
not wedge the agent.
"""

import sys

from lib import feedback, transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_state import bump_gate_block, gate_block_count, load_state

# This gate blocks at most this many times per turn, then yields for the rest of
# the turn — it flags an obvious problem once and trusts the agent after that.
GATE_BLOCK_CAP = 1

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
    "responded to. You do NOT judge whether the agent read or researched enough — a "
    "different check owns that. Judge only whether the last message is worth the "
    "architect's time. You only RAISE the concern in `reason`; you never prescribe a "
    "fix, an edit, or a next step — the main agent has the full context and owns that. "
    "Output structured JSON only."
)


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0

    state = load_state(session_id)
    if state.get("state") == "interview":
        return 0
    last_msg = field(event, "last_assistant_message", "")
    if not last_msg.strip():
        return 0

    # Already flagged this turn — yield rather than re-block the same stop, which
    # is what freezes the session.
    if gate_block_count(session_id, "babysitter") >= GATE_BLOCK_CAP:
        return 0

    transcript_path = field(event, "transcript_path", "")
    # Stopping to await dispatched work is an async pause, not skipped work — the
    # agent resumes when the dispatch wakes it. Don't gate that.
    if transcript.awaits_async_work(transcript.current_turn_lines(transcript_path)):
        return 0

    # Only the last turn: the architect's last message and this reply. babysitter
    # judges the reply's correctness, not the session's history.
    recs = transcript.records(transcript_path)
    request = transcript.clamp(transcript.recent_user_texts(recs, 1))
    current_state = state.get("state") or "proposing"

    result = run_model(system_prompt=SYSTEM_PROMPT,
                       user_prompt=_eval_prompt(request, last_msg, current_state),
                       schema=JSON_SCHEMA, session_id=session_id, hook="babysitter")
    if not result:
        return 0

    if result.get("allow", True) is False:
        reason = result.get("reason") or "the reply may not be optimized for the architect"
        bump_gate_block(session_id, "babysitter")
        return feedback.raise_concern("babysitter", "Stop", "Potential issue: %s" % reason)

    return 0


def _mode_line(current_state):
    """How to read the turn, so a proposal-mode decision is never punished as
    scope-tampering. The persisted state alone is not the mode — a question or a
    correction is an alignment turn even when state is executing."""
    return (
        "Session state: %s. Read the architect's last message to judge the turn:\n"
        "- If they are asking a question, correcting or refining your output, or "
        "state is 'proposing' — this is an ALIGNMENT turn. Surfacing a decision, "
        "presenting options, or asking a real architectural question IS the job. "
        "Never block it as scope-tampering or deferral.\n"
        "- Only when state is 'executing' AND they handed you approved work to "
        "implement does deferring, splitting, or asking permission count against "
        "the agent.\n---\n"
    ) % current_state


def _eval_prompt(request, last_msg, current_state):
    return (
        'Judge the agent\'s last message to the architect.\n\n'
        '%s'
        'The architect\'s last message (what this reply answers):\n%s\n---\n'
        'The agent\'s LAST MESSAGE (what the architect reads and acts on):\n%s\n---\n'
        'BLOCK the stop if ANY of these is true. The architect reads ONLY this message, '
        'with no other context, and will not read files, scroll back, or guess:\n\n'
        '1. UNDEFINED AGENT COINAGE — the message uses a term the AGENT invented for a concept the project already names, AND that term is opaque: its meaning is not clear from the message or the conversation (e.g. "hid" for "hidden: true", first use, unexplained). Block only then; name the term and tell it to use the project\'s word or plain English. Do NOT block a term, abbreviation, file nickname, or shorthand already used or established this session, or whose meaning is obvious in context — the architect has the conversation in mind and knows it; never demand a full file path for a file already discussed. Do NOT block for using the project\'s own vocabulary or for referring to its files. Never demand pasted code — line-by-line code is wrong unless the architect asked for code this turn.\n\n'
        '2. INCONSISTENT-TERMS — the message uses different words for one thing, or one word for different things, forcing the architect to disambiguate. Block; name the colliding terms.\n\n'
        '3. AMBIGUOUS — a sentence carries more than one reading, or its meaning is unclear. Block; quote the ambiguous sentence.\n\n'
        '4. ABSTRACTION-NOT-CONCRETE — the message proposes an abstraction instead of a concrete, reviewable change. The architect cannot approve "a backend factory"; they can approve "backend/Utils/UserFactory.php — a new file that centralizes all user creation": the concrete path, name, and purpose. Block any proposed file, class, method, or change named only as a category/abstraction without its concrete path, name, and purpose. This includes answering a request for the fixes or changes with a shortlist of DIRECTIONS ("make it fail loud", "bound the memory", "build from the deployed commit") instead of the exact edits — a direction the architect must himself turn into a reviewable change costs him a full round-trip. Block it.\n\n'
        '5. NOT-STRUCTURED — decisions and information are buried in undifferentiated prose instead of organized (headings, lists, tables, a diagram, a file tree) so a human can parse them fast. Block.\n\n'
        '6. CHAIN-OF-THOUGHT-DUMP — the message reads as the agent\'s own exploration/thinking log rather than a response fine-tuned to the architect\'s time. Reasoning belongs in thinking, not the reply. Block.\n\n'
        '7. WASTES-TIME — the message repeats itself, restates a prior answer the architect already responded to, or pads with content that adds nothing (a recap of settled context, history he said he does not care about). A thorough, on-topic proposal or a complete answer to exactly what he asked is NOT waste for being long — judge whether the length carries decision-relevant content, never whether it is short. The user\'s last message being terse (a one-line constraint or correction) does not make a full, on-point proposal in reply too long. Block only genuine repetition or padding.\n\n'
        '8. PADDED-OPTIONS — the message presents a /pcc or options block where one option is filler: it has no honest pro, is worse on every axis, or exists only so there can be a second option. A fake option wastes the architect\'s read and fakes rigor. Block; name the filler option and tell the agent to find a real alternative or present the one good path alone. A GENUINE alternative with a real, distinct con and its own confidence is NOT filler, even when its confidence is lower — distinct-confidence options are exactly how a proposal surfaces a real decision for the architect to pick. Only block when an option truly has no honest pro or is strictly worse on every axis; never block a proposal merely for offering a lower-confidence second path. An option whose con openly states why it loses — it violates a ruled-out constraint, it tests the wrong state — is the agent showing its work, honest rigor that spares the architect re-proposing that path, NOT filler. Do not flag it.\n\n'
        '9. SCOPE-TAMPERING (executing turns only) — when state is \'executing\' and the architect handed over approved work, the agent UNILATERALLY tried to change, narrow, break down, defer, fragment, or reorder that scope on its own initiative: work the architect told it to do that it pushed later or split across turns by itself. Block agent-invented "for now", "as a follow-up", "should I do A or B first", or splitting approved work across turns. But when the ARCHITECT himself set the scope as a subset — named specific files or an area, said "just X" / "only this", or warned the rest of the tree is his own separate work — doing exactly that subset and REPORTING what was deliberately left out is honoring scope, never tampering: ALLOW it. A scoped commit that excludes unrelated in-flight changes and names what it excluded and why is correct hygiene, not deferral. The failure is the agent shrinking the architect\'s work; it is never the agent honoring a boundary the architect drew. On alignment turns (question, correction, proposing), this rule does NOT apply — presenting a decision is correct.\n\n'
        '10. REQUIRES-MEMORY — the message makes the architect remember something: it references an earlier reply, an earlier decision, or code without explaining it in plain terms; states a conclusion whose basis it never gives; or states something out of context without providing the context. Block. The fix is to carry the needed context inline in plain architectural language — not to paste code.\n\n'
        '11. UNGROUNDED-FACT — the message states a fact about the architect\'s OWN stack, code, data, or tools that the agent did not verify and that the architect can disprove by knowing his own project (e.g. "Action Scheduler is bundled inside WooCommerce" when there is no WooCommerce). A confident wrong fact about his system is worse than silence — he acts on it. Block; name the unverified claim and tell the agent to verify it against the actual source before stating it. Do NOT block a claim the message shows it grounded (cites the file, the value, the config it read). This rule targets a factual ASSERTION stated in prose (e.g. "Action Scheduler is bundled in WooCommerce"). A code or config excerpt the agent shows to illustrate how something works is it displaying the mechanism, not a prose claim about the stack — never block shown code under this rule; the architect wants to see the code and judges it himself. This is about the state of his EXISTING system — what it contains, how it works, what depends on what. The agent reporting its own actions this turn — what it created, changed, or ran, including the files an approved task obviously required — is a completion report, not a claim about his system; never apply this rule to it, and never block reported work because it looks like it should have been escalated.\n\n'
        '12. HANDED-BACK-DECISION — on a proposing/alignment turn, the message hands the architect a question, fork, or choice the agent could have resolved itself by reading the code, or asks him to supply knowledge the developer should produce. The tells are punting phrases — "that\'s your call to make", "you have to decide the shape", "how should I know", "do you want me to X or Y" — on something the agent should have settled. Block; tell it to read the code, resolve what the code answers, and surface only what genuinely needs his decision. A GENUINE decision the code cannot settle — a real architectural fork, a real external-context gap — presented as resolved options with distinct confidences is NOT this and must be ALLOWED; the failure is punting an answerable thing, never surfacing a real one.\n\n'
        '13. CONTRADICTS-GOAL — the message gives a recommendation or conclusion whose own stated effect runs against the goal it serves (e.g. recommends restoring update checks "to improve performance" when the checks add work). Block; name the contradiction and tell the agent to make every recommendation consistent with its goal.\n\n'
        '14. CODE-DUMP-NOT-ARCHITECTURE — the message makes the architect read code that serves no purpose for the decision in front of him: implementation pasted with no reason he needs it, or code-level detail (function bodies, line numbers) inside an architecture-level decision. Test: remove the code — can he still make the decision? If yes, the code was decoration; block, and tell the agent to lead with the architecture (what changes, what it affects, the impact), keeping only the code the decision turns on. If no — the decision genuinely turns on seeing this exact code — it is essential and passes. Code with a purpose always passes: a snippet showing how a mechanism works, a documentation or prose diff, a before/after that makes a subtle change concrete, an answer to a question about code. This rule never reduces the code he gets; it ensures the code he gets is code he needed.\n\n'
        '15. QUESTION-NOT-ANSWERED — the architect\'s last message is a question (especially "why X?"), and the reply does not answer it: it acknowledges and goes back to editing, proposes or plans a change, or says what it will do, instead of investigating and giving the answer. A question is answered, not acted on. Block; tell the agent to answer the question (trace the code if it must) and make no change until he asks for one. Do NOT fire when the reply actually answers the question — a direct, grounded answer to what he asked is correct and must be ALLOWED.\n\n'
        'ALLOW the stop when the message is self-contained, concrete (real paths, names, purposes), '
        'structured for fast parsing, faithful to the architect\'s scope, and explains every reference '
        'inline in plain architectural terms (purpose, files, data, public API) — not pasted code — or '
        'when it is a short, clear, fully self-contained factual answer to a direct question, or when '
        'the agent is pausing to await background subagents it dispatched this turn (a legitimate async '
        'wait, not skipped work), or when on an alignment turn it surfaces a GENUINE decision or options '
        'it has resolved as far as the code allows, for the architect to choose — not a fork the code '
        'answers handed back unresolved.\n\n'
        'Return JSON:\n'
        '- Allow: {"allow": true}\n'
        '- Block: {"allow": false, "reason": "the rule + the specific offending part"}'
    ) % (_mode_line(current_state), request, last_msg)


if __name__ == "__main__":
    sys.exit(main())
