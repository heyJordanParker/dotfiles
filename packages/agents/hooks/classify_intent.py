#!/usr/bin/env python3
"""Classify user intent and inject contextual guidance (LLM check).

On every UserPromptSubmit it:
classifies INTENT via a separate `claude -p` instance, runs a deterministic state
machine (proposing | executing | auto), applies typed mode-command overrides
(/propose /execute force STATE; /solo /subagents /team force APPROACH; /commit
forces a commit), persists
the result to the session spine (the control state the proposal/commit guards
read), and emits injected context as a UserPromptSubmit additionalContext envelope.

Any infrastructure failure (recursion guard, missing tool, parse error, LLM
unavailable) returns 0 and never blocks the user's prompt. A typed mode command
still takes effect via the fallback path even when the LLM is unreachable.

The state machine transitions STATE deterministically from the LLM's INTENT; the
LLM only outputs intent + approach/state mutation hints + extracted instructions.
"""

import json
import os
import re
import sys

from lib import transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_state import load_state, merge_state

BINDING = {
    "events": {"UserPromptSubmit": []},
    "timeout": 60,
    "harness": "all",
}


def emit_context(text):
    sys.stdout.write(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": text,
        }
    }) + "\n")


# --- structural system-message detection --------------------------------------

def is_system_prompt(prompt):
    # XML-tagged: starts with <tag>, contains matching </tag>
    if prompt.startswith("<"):
        tag_rest = prompt[1:]
        tag_name = re.split(r"[> ]", tag_rest, maxsplit=1)[0]
        if tag_name and ("</%s>" % tag_name) in prompt:
            return True
    # Bracket-enclosed: entire message is a single [...] line (no content after)
    if prompt.startswith("["):
        first_line = prompt.split("\n", 1)[0]
        if first_line.endswith("]") and prompt == first_line:
            return True
    # Raw text system messages (rare, no structural signal)
    if prompt.startswith("This session is being continued"):
        return True
    if prompt.startswith("Base directory for this skill:"):
        return True
    return False


# --- typed mode-command detection ----------------------------------------------

def _typed(prompt, token):
    """Bounded literal-token match: /propose matches, /proposed and /solos don't.

    The token must sit on a non-alphanumeric boundary, evaluated against the
    prompt padded with a leading and trailing space.
    """
    padded = " %s " % prompt
    pattern = r"(^|[^A-Za-z0-9])" + re.escape(token) + r"($|[^A-Za-z0-9-])"
    return re.search(pattern, padded) is not None


def forced_commands(prompt):
    forced_state = ""
    if _typed(prompt, "/propose"):
        forced_state = "proposing"
    elif _typed(prompt, "/execute"):
        forced_state = "executing"
    forced_approach = ""
    if _typed(prompt, "/solo"):
        forced_approach = "solo"
    elif _typed(prompt, "/subagents"):
        forced_approach = "subagents"
    elif _typed(prompt, "/team"):
        forced_approach = "team"
    forced_commit = _typed(prompt, "/commit")
    return forced_state, forced_approach, forced_commit


# --- transcript conversation-context extraction --------------------------------

def conversation_context(transcript_path):
    stream_lines = transcript.conversation_stream(transcript.records(transcript_path))
    if not stream_lines:
        return ""

    # Turn boundary: 1-based line number of the last human (U|) message.
    turn_line = 0
    for idx, line in enumerate(stream_lines, start=1):
        if line.startswith("U|"):
            turn_line = idx

    recent_turns = ""
    agent_response = ""
    if turn_line > 0:
        after = stream_lines[turn_line:]  # lines after the boundary
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
        parts = ("Recent conversation (background — do NOT extract instructions "
                 "from this section):\n%s\n\n---\n" % recent_turns)
    if agent_response:
        parts = ("%sAgent's last response (what the user is directly responding "
                 "to):\n%s\n\n---\n" % (parts, agent_response))
    return parts


# --- LLM classifier flags ------------------------------------------------------

JSON_SCHEMA = '{"type":"object","properties":{"intent":{"type":"string","enum":["approval","question","instructions","correction","proposal_request"]},"instructions":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"},"mode":{"type":"string","enum":["question","execute","correction"]}},"required":["text","mode"]}},"skills":{"type":"array","items":{"type":"string"}},"approach_change":{"type":"string","enum":["solo","subagents","team","no_change"]},"state_change":{"type":"string","enum":["proposing","executing","auto","no_change"]},"sequential":{"type":"boolean"},"commit_requested":{"type":"boolean"},"session_notes":{"type":"array","items":{"type":"string"}},"recommended_agents":{"type":"array","items":{"type":"object","properties":{"agent":{"type":"string"},"reason":{"type":"string"}},"required":["agent","reason"]}}},"required":["intent"]}'

SYSTEM_PROMPT = "You are a classifier. Classify user intent. Extract instructions. Output structured JSON only."


def evaluation_prompt(prompt, session_notes_context, conversation_block,
                      current_approach, current_state):
    return (
'Classify this user message\'s INTENT and extract any specific instructions, requirements, or constraints.\n'
'\n'
'%s%sCurrent message to classify:\n'
'%s\n'
'\n'
'You are classifying INTENT only. The state machine transitions are handled separately — you do not decide the agent\'s state.\n'
'\n'
'Intent types:\n'
'- "approval" — user is approving/accepting a specific proposal or plan that the agent just presented. Requires a preceding proposal to approve. Message can be pure ("yes", "go ahead") OR compound — an approval/acknowledgement signal followed by imperative scope directing execution of the just-proposed work ("okay, update those using /cc", "perfect, commit this with /commit-message"). In compound form, intent stays "approval" AND the imperative tail is extracted into instructions[] with mode: "execute" — the approval signal is primary, the imperative is scope attached to it, not a separate directive. "just fix it" or "do it" as a standalone directive without a preceding proposal → instructions, not approval. An approval lead word followed by a pivot to unrelated new work ("okay, now let\'s do X instead") → instructions — the lead word only carries approval weight when the remainder references the standing proposal.\n'
'- "question" — user is asking a question that requires an answer. No action should be taken.\n'
'- "instructions" — user is giving the agent new work to do with action language ("fix", "add", "change", "implement", "update", "remove", "refactor"). Also standalone constraints and boundaries ("don\'t use third-party libraries") when there is no previous agent output being refined.\n'
'- "correction" — user is correcting or giving feedback on previous agent output ("that\'s wrong", "no, use X instead", "this is fine", "this is a non-issue"). Requires previous agent output being refined — a standalone constraint like "don\'t use third-party libraries" without the agent having proposed or used them is instructions (a boundary), not correction.\n'
'- "proposal_request" — user is asking for analysis, investigation, review, or a proposal BEFORE execution ("propose", "analyze", "what would we need", "design", "plan", "evaluate", "compare options", "investigate", "look into", "dig into", "figure out", "see what\'s going on", "check why", "double check", "verify", "review", "audit", "validate").\n'
'\n'
'Rules:\n'
'0. COMPOUND APPROVAL + IMPERATIVE SCOPE is the dominant approval shape — users rarely send a bare "yes"; they approve and add scope in one breath. When a message opens with an approval/acknowledgement lead word ("okay", "yes", "sure", "yeah", "alright", "go ahead", "perfect", "great", "fine", "approved", "let\'s", "cool", "nice") AND the remainder contains imperative verbs referring to work the agent just proposed or discussed, classify as "approval" — NOT "instructions". Extract the imperative remainder into instructions[] with mode: "execute". Both the approval signal AND the executable scope are preserved in the output. Failure mode to avoid: dropping the lead word as throat-clearing and classifying the remainder as pure "instructions" — this locks the session in propose-first mode and blocks the edits the user just approved.\n'
'1. A message that makes sense as a response to the last AI output IS a response to it — classify based on what it\'s responding to, not its surface form. A single word after an options list is a selection (approval). A short reply after a proposal is feedback (correction or approval).\n'
'2. Emotional language is emphasis, not a separate category. Strip the emotion, classify the underlying intent.\n'
'3. Corrections preserve the current direction. A message that corrects previous output while also containing forward-looking language ("no, just use X and then deploy it") is still a correction — the user is refining, not giving new independent instructions. Messages that add scope to an active proposal or discussion ("one more thing:", "also include", "we also need", "don\'t forget about") are corrections when the user is refining what should be proposed. Only classify as "instructions" when the message introduces genuinely new work unrelated to the previous output.\n'
'4. When uncertain between "instructions" and "proposal_request", prefer "proposal_request". The conservative default is to propose before executing.\n4b. When a correction also redirects to a fundamentally new direction ("that won\'t work, propose something with Redis instead", "scrap this, try a different approach"), classify as "proposal_request" — the user wants a fresh proposal, not a patched version of the rejected one.\n4c. When the user answers questions from a proposal ("yes", "Option A", "obviously"), classify as "correction" — the user is refining the proposal with their answers, not approving execution. Only classify as "approval" when the user explicitly signals the proposal is complete and ready for execution.\n4d. A directive to improve or change code that names no concrete change — "make X better", "X needs to handle Y", "improve X", "X should be more robust" — is "proposal_request", not "instructions". The absence of a specific change is the signal to propose first.\n'
'5. Set "sequential" to true when the user\'s instructions have explicit ordering ("after that", "then", "finally", numbered steps with dependencies). Default to false.\n'
'6. All user instructions contain subtleties and nuance — preserve ordering language, execution context, autonomy cues, and boundary conditions verbatim. Never flatten, summarize, or strip nuance from extracted instructions. Each instruction is an object with "text" (the instruction) and "mode" (one of: "execute" for actions, "question" for things the user wants answered, "correction" for feedback on previous analysis). When a message contains BOTH a question AND an action directive, extract both with their respective modes — never absorb an action into a question or vice versa. Example: "how does X work? also change Y to Z" → [{"text": "how does X work", "mode": "question"}, {"text": "change Y to Z", "mode": "execute"}].\n'
'6b. Questions in the user\'s message require direct answers. Extract every question as a separate mode: "question" instruction, even when embedded in corrections, emotional language, or rhetorical framing. "What is this about?" is a question. "Do we have this elsewhere?" is a research question. "Did you read X?" is a question requiring an honest answer. Do not dismiss questions as decorative or rhetorical.\n'
'7. Only include a skill in "skills" if the user is invoking it — telling the agent to use or execute it NOW. If the skill is discussed, referenced, or mentioned as context, do not include it. "use /commit" → include. "the /subagents skill handles this" → exclude. When in doubt, include it — a false positive is cheaper than a false negative.\n'
'8. Set "approach_change" to "no_change" unless the user signals an approach shift. Two trigger families:\n(a) Direct mode-change requests — always trigger transition. The user names the target mode using forms like "enter X mode", "go X" / "go to X mode", "switch to X", "X mode", where X is one of solo, subagents, team. Negation/inverse forms also count when they unambiguously name a target. Direct mode-change requests are a HARD OVERRIDE — they fire even when buried inside long messages, repeated for emphasis, or wrapped in frustration. The user\'s literal naming of the target mode is dispositive; absence of dispatch verbs does not block the transition.\n  - "solo" on: "go solo", "enter solo mode", "solo mode", "switch to solo", "do this yourself", "don\'t spawn agents", "read it yourself".\n  - "subagents" on: "enter subagents mode", "go subagents", "go to subagents", "go to subagents mode", "switch to subagents", "subagents mode", "exit solo", "use agents", "use subagents".\n  - "team" on: "enter team mode", "go team", "go to team mode", "switch to team", "team mode", "get a team", uses /team.\n(b) Dispatch-language signals — trigger ONLY when the new approach differs from current. "subagents" when the user asks to launch, spawn, dispatch, or use agents/subagents in this turn (e.g. "spawn a subagent", "launch 3 agents in parallel", "have a @debugger investigate", "get an agent to do X", "1 subagent to research Y"). Output "no_change" if current approach is already "subagents" or "team" — these phrases describe work within the existing approach, not a shift.\nSingle-action directives without dispatch language ("run the tests", "fix the bug", "add a guard", "commit this") are NOT approach signals. Mentioning a specific agent (e.g. "get a @debugger") is NOT a "team" signal — "team" requires explicit team language. It IS a "subagents" signal when current approach is "solo". Current approach: %s.\n'
'8b. Set "state_change" to "no_change" unless the user signals a state shift. Two trigger families:\n(a) Mode-name declarations — always trigger transition. "executing" on "execute mode", "enter execute mode", "go into execute mode". "proposing" on "proposing mode", "go back to proposing", "propose first". "auto" on "auto mode", "mixed mode".\n(b) Execution-intent signals — trigger "executing" when the user\'s phrasing rules out a propose-first interpretation: "execute this" / "execute X" / "execute the plan", "just do it" / "just execute" / "skip the proposal", "implement this directly" / "build it now" / "ship it" / "do it now", or execution paired with dispatch ("spawn a subagent to implement X", "launch agents to fix Y", "get an agent to build Z").\nConservative default: if the message could reasonably be read as a request to research, investigate, or propose, output "no_change" and let the state machine apply its propose-first bias. Only output "executing" when the phrasing explicitly demands action NOW.\nThis is a HARD OVERRIDE — output wins over intent classification and current state. Single-action directives without execution-intent language ("fix the bug", "commit this", "deploy") are NOT state signals — they\'re work instructions handled by the state machine. Current state: %s.\n'
'9. Set "commit_requested" to true when the user explicitly asks for a git commit — "commit this", "/commit", "create a commit". Approving work, applying changes, deploying, replacing files, shipping — none of these are commit requests. If the user doesn\'t say "commit", commit_requested is false.\n'
'10. Add to "session_notes" ONLY when this message reveals a surprise — the agent did something illogical that confused the user, the user explicitly forbids something with always/never language, the user corrects the same behavior twice, or the user\'s emotional state (frustration, anger, exasperation) indicates the agent surprised them. Maximum 10 notes total (including existing). If adding would exceed 10, drop the least critical existing note. Return the FULL list of notes (existing + new) in session_notes, or an empty array if no changes.\n'
'11. Set "recommended_agents" when the user\'s intent clearly matches a specialized agent. Only include agents that clearly match — empty array when no match is obvious. Multiple recommendations are fine when the task spans domains.\n'
'12. The conversation history is BACKGROUND CONTEXT for understanding what the user is responding to. Extract instructions ONLY from the current message. If the current message narrows, changes, or contradicts earlier messages, follow the current message — it represents the user\'s latest position. Never synthesize instructions by combining multiple older messages.\n'
'\n'
'Agent routing — recommend when user intent matches:\n'
'- Claude.md, skills, hooks, plugins, context engineering, documentation → context-engineer\n'
'- Architecture, system design, encapsulation, dependency direction → architect\n'
'- Bugs, test failures, errors, stack traces, "doesn\'t work" → debugger\n'
'- Code quality, diff review, PR review → code-reviewer\n'
'- UI components, CSS, styling, layouts, visual design → designer\n'
'- Frontend features, React, user flows, UX implementation → frontend-engineer\n'
'- Backend features, API, database, services → backend-engineer\n'
'- UX testing, "test the flow", browser testing → ux-tester\n'
'- Feature verification, API testing, "does this work" → tester\n'
'- External docs, library research, "how does X work" → researcher\n'
'\n'
'Examples:\n'
'- "yes" → {"intent": "approval", ...}\n'
'- "go ahead, also fix the related issue" → {"intent": "approval", "instructions": [{"text": "also fix the related issue", "mode": "execute"}], ...}\n'
'- "okay, update those using /cc" → {"intent": "approval", "instructions": [{"text": "update those using /cc", "mode": "execute"}], "skills": ["/cc"], ...}\n'
'- "sure, and also update our /cc references to mention those conditionals" → {"intent": "approval", "instructions": [{"text": "also update our /cc references to mention those conditionals", "mode": "execute"}], "skills": ["/cc"], ...}\n'
'- "perfect, commit this with /commit-message" → {"intent": "approval", "instructions": [{"text": "commit this with /commit-message", "mode": "execute"}], "skills": ["/commit-message"], ...}\n'
'- "approved\\n\\nwhen done, commit with /commit-message and push" → {"intent": "approval", "instructions": [{"text": "when done, commit with /commit-message and push", "mode": "execute"}], "skills": ["/commit-message"], ...}\n'
'- "yeah, reread the /cc skill & then update" → {"intent": "approval", "instructions": [{"text": "reread the /cc skill & then update", "mode": "execute"}], "skills": ["/cc"], ...}\n'
'- "yes. make sure you don\'t touch the tmux infrastructure at all; we are NOT migrating" → {"intent": "approval", "instructions": [{"text": "make sure you don\'t touch the tmux infrastructure at all; we are NOT migrating", "mode": "execute"}], ...}\n'
'- "fine. implement & test this; I want to see it completely working after I\'m back" → {"intent": "approval", "instructions": [{"text": "implement & test this; I want to see it completely working after I\'m back", "mode": "execute"}], ...}\n'
'- "go ahead; use the bricks skill if you have bricks specific work... execute" → {"intent": "approval", "instructions": [{"text": "use the bricks skill if you have bricks specific work... execute", "mode": "execute"}], ...}\n'
'- "okay, now let\'s do X instead" → {"intent": "instructions", ...}  (negative case — lead word does NOT carry approval when the remainder pivots to unrelated new work)\n'
'- "why does X work this way?" → {"intent": "question", ...}\n'
'- "fix the bug in MediaController" → {"intent": "instructions", "instructions": [{"text": "fix the bug in MediaController", "mode": "execute"}], ...}\n'
'- "that\'s wrong, use Y instead" → {"intent": "correction", "instructions": [{"text": "use Y instead", "mode": "correction"}], ...}\n'
'- "propose a fix for the auth bug" → {"intent": "proposal_request", "instructions": [{"text": "propose a fix for the auth bug", "mode": "execute"}], ...}\n'
'- "analyze this and tell me what\'s wrong" → {"intent": "proposal_request", "instructions": [{"text": "analyze this and tell me what\'s wrong", "mode": "execute"}], ...}\n'
'- "investigate why the tests are failing" → {"intent": "proposal_request", ...}\n'
'- "double check your work" → {"intent": "proposal_request", ...}\n'
'- "make the classifier handle compound approvals better" → {"intent": "proposal_request", ...}\n'
'- "just fix it" → {"intent": "instructions", "commit_requested": false, ...}\n'
'- "deploy this to production" → {"intent": "instructions", "commit_requested": false, ...}\n'
'- "commit this" → {"intent": "instructions", "commit_requested": true, ...}\n'
'- "how does X work? also change Y to Z" → {"intent": "question", "instructions": [{"text": "how does X work", "mode": "question"}, {"text": "change Y to Z", "mode": "execute"}], ...}\n'
'\n'
'Detect /skill references (slash followed by a name, e.g. /commit, /review, /ask) — only include if being invoked, not discussed. Preserve the leading slash in skill names ("/commit" not "commit"). A bare slash-command as the entire message is always an invocation.'
    ) % (session_notes_context, conversation_block, prompt, current_approach, current_state)


def run_classifier(prompt_text, session_id=""):
    """Returns the classified intent dict, or None on any failure."""
    return run_model(SYSTEM_PROMPT, prompt_text, JSON_SCHEMA,
                     session_id=session_id, hook="classify_intent")


# --- fallback (LLM unavailable but a typed command was given) -------------------

def fallback_context(forced_state, forced_approach):
    fallback = ""
    if forced_state == "executing":
        fallback = ("This is an executing-state turn. Load the /execute skill now and "
                    "work under its contract: implement the approved work, and the moment "
                    "it needs an architectural change, stop and escalate with /pcc.")
    elif forced_state == "proposing":
        fallback = ("This is a proposing-state turn. Load the /propose skill now and "
                    "produce the proposal under its contract.")
    approach_line = {
        "solo": "Load the /solo skill now.",
        "subagents": "Load the /subagents skill now.",
        "team": "Load the /team skill now.",
    }.get(forced_approach, "")
    if approach_line:
        fallback = (fallback + "\n\n" + approach_line) if fallback else approach_line
    return fallback


COMMIT_DIRECTIVE = (
    "Skills to execute: /commit-message\n\nAfter completing the commit, review "
    "session notes and suggest which should become permanent — in global/project "
    "Claude.md, skills, agents, rules, or commands as appropriate. Present "
    "suggestions only, do not act on them."
)


# --- instruction formatting ----------------------------------------------------

def _instructions(result):
    inst = result.get("instructions")
    return inst if isinstance(inst, list) else []


def _numbered_by_mode(result, mode):
    items = [i for i in _instructions(result)
             if isinstance(i, dict) and i.get("mode") == mode]
    return "\n".join("%d. %s" % (n, i.get("text", "")) for n, i in enumerate(items, 1))


def _numbered_all(result):
    out = []
    for n, i in enumerate(_instructions(result), 1):
        if isinstance(i, dict):
            text = i.get("text", i)
        else:
            text = i
        out.append("%d. %s" % (n, text))
    return "\n".join(out)


def _execute_count(result):
    return len([i for i in _instructions(result)
                if isinstance(i, dict) and i.get("mode") == "execute"])


def main():
    # Guard against recursion
    if os.environ.get("CLAUDE_SESSION_HOOK") == "true":
        return 0

    event = read_event()

    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0

    prompt = field(event, "prompt", "")
    if not prompt:
        return 0

    if is_system_prompt(prompt):
        return 0

    transcript_path = field(event, "transcript_path", "")

    state = load_state(session_id)
    current_approach = state.get("approach") or "solo"
    current_state = state.get("state") or "proposing"
    current_intent = state.get("intent") or "instructions"
    current_notes = state.get("notes")
    if not isinstance(current_notes, list):
        current_notes = []

    # Reset validation phase on new user message; a standalone write so the
    # reset still lands when classification fails before the final write below.
    merge_state(session_id, {"validation_phase": 0})

    forced_state, forced_approach, forced_commit = forced_commands(prompt)

    # Format existing notes for classifier context
    session_notes_context = ""
    if current_notes:
        formatted = "\n".join("- %s" % n for n in current_notes)
        if formatted:
            session_notes_context = (
                "\nActive session notes (behavioral corrections from this session — "
                "these shift how you classify):\n%s\nCurrent approach: %s\nCurrent state: "
                "%s\n\n---\n" % (formatted, current_approach, current_state)
            )

    conversation_block = conversation_context(transcript_path)

    eval_prompt = evaluation_prompt(
        prompt, session_notes_context, conversation_block,
        current_approach, current_state,
    )

    result = run_classifier(eval_prompt, session_id)

    # If classification failed for any reason, pass through. But a typed mode command
    # is deterministic and must still take effect — persist it and emit its contract.
    if not result:
        if forced_state or forced_approach or forced_commit:
            forced = {}
            if forced_state:
                forced["state"] = forced_state
            if forced_approach:
                forced["approach"] = forced_approach
            if forced_commit:
                forced["commit_requested"] = True
            if forced:
                merge_state(session_id, forced)
            fallback = fallback_context(forced_state, forced_approach)
            if forced_commit:
                fallback = (fallback + "\n\n" + COMMIT_DIRECTIVE) if fallback else COMMIT_DIRECTIVE
            if fallback:
                emit_context(fallback)
        return 0

    intent = result.get("intent") or "instructions"

    execute_instructions = _numbered_by_mode(result, "execute")
    question_instructions = _numbered_by_mode(result, "question")
    correction_instructions = _numbered_by_mode(result, "correction")
    instructions_all = _numbered_all(result)

    skills_list = result.get("skills")
    skills = ", ".join(skills_list) if isinstance(skills_list, list) else ""

    approach_change = result.get("approach_change") or "no_change"
    # Apply approach mutation — typed command wins, then LLM transition, then current
    if forced_approach:
        approach = forced_approach
    elif approach_change != "no_change":
        approach = approach_change
    else:
        approach = current_approach
    # Auto-invoke matching skill on approach transition (mirrors per-mode skills)
    if approach != current_approach:
        auto_skill = {"solo": "/solo", "subagents": "/subagents",
                      "team": "/team"}.get(approach, "")
        if auto_skill and auto_skill not in skills:
            skills = ("%s, %s" % (skills, auto_skill)) if skills else auto_skill

    sequential = bool(result.get("sequential")) if result.get("sequential") is not None else False
    commit_requested = bool(result.get("commit_requested")) if result.get("commit_requested") is not None else False
    if forced_commit:
        commit_requested = True
    new_notes = result.get("session_notes")
    if not isinstance(new_notes, list):
        new_notes = []
    recommended = result.get("recommended_agents")
    if isinstance(recommended, list):
        recommended_agents = "\n".join(
            "- @%s — %s" % (a.get("agent", ""), a.get("reason", ""))
            for a in recommended if isinstance(a, dict)
        )
    else:
        recommended_agents = ""

    # ==========================================================================
    # Deterministic state transition — LLM classifies intent, code decides state
    # ==========================================================================
    execute_count = _execute_count(result)

    new_state = current_state
    if intent == "proposal_request":
        new_state = "proposing"
    elif intent == "instructions":
        if current_state != "proposing":
            new_state = "executing"
    elif intent == "approval":
        new_state = "executing"
    elif intent == "question":
        if execute_count > 0 and current_state != "proposing":
            new_state = "auto"
    elif intent == "correction":
        if execute_count > 0 and current_state != "proposing":
            new_state = "auto"

    # Hard override: explicit state declaration from user wins over the derivation
    state_change = result.get("state_change") or "no_change"
    if state_change != "no_change":
        new_state = state_change

    # Typed /propose or /execute is the top priority — over intent and over the LLM.
    if forced_state:
        new_state = forced_state

    # Detect state transitions for agent notification
    state_notifications = ""
    if approach != current_approach:
        state_notifications += ("Session state updated: approach changed from '%s' to '%s'.\n"
                                % (current_approach, approach))
    if new_state != current_state:
        state_notifications += ("Session state updated: state changed from '%s' to '%s'.\n"
                                % (current_state, new_state))
    if intent != current_intent:
        state_notifications += ("Session state updated: intent changed from '%s' to '%s'.\n"
                                % (current_intent, intent))
    if commit_requested:
        state_notifications += "Session state updated: commit authorized.\n"

    # Update session control state on the spine
    save_notes = new_notes if new_notes else current_notes
    merge_state(session_id, {
        "approach": approach,
        "state": new_state,
        "intent": intent,
        "commit_requested": commit_requested,
        "notes": save_notes,
        "validation_phase": 0,
    })

    # ==========================================================================
    # Build context based on intent and state
    # ==========================================================================
    context = ""
    if intent == "approval":
        restatement_target = "what was discussed"
        context = "Approval. Start work on what was just discussed."
        if execute_instructions:
            context += "\n\nAdditional scope from user:\n" + execute_instructions
    elif intent == "question":
        restatement_target = "the question"
        if execute_count > 0:
            context = ("This is a question with action items. Answer the question AND "
                       "execute the action items.")
        else:
            context = ("This is a question. Answer it.\n\n- Don't edit the code. Don't make "
                       "decisions based on a question. Don't assume intent.")
        context += (
            "\n- Don't be a sycophant. No hedging. No \"you're right, the problem is…\" after "
            "a question. No reframing, validating, or characterizing the question — just answer it."
            "\n- Don't guess what the user wants or means. Don't infer feedback from questions. A "
            "question is not a complaint or a critique — never respond with \"you're right to "
            "question this.\"\n- Don't exit plan mode.\n- Don't update the plan.\n- Never bias and "
            "direct the user with your reply. Objectively report the facts & only the facts.\n- "
            "Directly answer with the root cause and the architectural decisions that led here.\n- "
            "Never ask questions the code can answer — read the code first, then answer.\n- "
            "Zero-guess policy — every code assertion must be validated against the source before "
            "it is claimed. If the answer depends on what the code does, returns, contains, or "
            "causes, reading the relevant source is mandatory before answering. No statement about "
            "code without reading it. Pattern matching is not validation. The correct answer beats "
            "the quick answer — your instinct will be to answer fast; resist it. Never trade rigor "
            "for speed.\n- Focus – answer EXACTLY what was asked & provide the necessary context.\n- "
            "Stay consistent – Jordan's word is gospel; don't forget.\n\nWhen presenting options or "
            "answering questions, use /pcc skill: architecturally distinct options, each with pros, "
            "cons, and confidence percentage. For yes/no questions, present the case for both sides. "
            "No hedging — state confidence as a percentage.\n\nBut not every question warrants "
            "options. These question shapes take a direct answer — do NOT force /pcc:\n- Why / "
            "reasoning: \"why did we pick X over Y\", \"why do we need this abstraction\", \"why is "
            "there duplication between X and Y\"\n- Verification / yes-no: \"have we handled the null "
            "case for X\", \"are we already using library Y somewhere\", \"have we already solved this "
            "in module N\"\n\nUse /pcc only when the user is choosing between REAL architectural "
            "alternatives — fundamentally different mechanisms, boundaries, data flows, or "
            "dependencies. Micro-decisions (file placement, naming, single-call refactors, log "
            "message wording, mechanical tweaks) get a direct answer, not an options list.\n\n/pcc "
            "requires 2+ viable options. If you only have one viable approach, present it as the "
            "answer itself — no pros, no cons, no confidence percentage. A single option is not a "
            "recommendation; recommendations rank multiple options. Never wrap a lone option in the "
            "/pcc format.\n\nPros describe how the option solves the stated problem. Cons describe "
            "real costs or risks the option introduces. Forbidden in cons: cross-option references "
            "(\"more complex than Option Z\"), treating normal implementation cost as inherent badness "
            "(\"8-file edit\"), filler added to balance the format. If an option has no real con, say "
            "so.\n\nConfidence ranks rightness — how confident you are this option is the right call "
            "for the stated problem, accounting for compromises. Major compromises drag the score "
            "down. Options clustered within ~10% (88/90/92) mean you haven't actually differentiated "
            "them.\n\nInconsistent confidence scores or pros/cons that feel forced are a signal of "
            "lacking codebase research. Fix them by reading more code, not by adjusting numbers or "
            "reshuffling bullets. Never ship a /pcc with patched-over scores — go back and research "
            "until the differences are clear."
        )
    elif intent == "correction":
        restatement_target = "these corrections"
        context = ""
        if correction_instructions:
            context = ("Corrections from user (acknowledge, do not change direction):\n"
                       + correction_instructions)
        if execute_instructions:
            if context:
                context += "\n\n"
            context += "Instructions from user:\n" + execute_instructions
        if question_instructions:
            if context:
                context += "\n\n"
            context += ("Questions from user (answer these, do not act on them):\n"
                        + question_instructions)
        if not context:
            context = "Corrections from user:\n" + instructions_all
        context += ("\n\nThe user corrected your previous output. Incorporate the correction "
                    "and deliver a complete response in the same format as the original — not "
                    "prose diffs. Any unresolved questions from the previous proposal must be "
                    "re-surfaced until answered.")
    elif intent in ("instructions", "proposal_request"):
        if question_instructions:
            restatement_target = "the questions and instructions"
        else:
            restatement_target = "these instructions"
        context = ""
        if question_instructions:
            context = ("The user asked questions that must be answered. Answer them directly:\n"
                       + question_instructions)
        if correction_instructions:
            if context:
                context += "\n\n"
            context += ("Corrections from user (acknowledge, do not change direction):\n"
                        + correction_instructions)
        if execute_instructions:
            if context:
                context += "\n\n"
            context += "Instructions from user:\n" + execute_instructions
        if not context:
            context = "Instructions from user:\n" + instructions_all
        if skills:
            context += "\n\nSkills to execute: " + skills
    else:
        restatement_target = "these instructions"

    # Sequential execution context
    if sequential:
        context += ("\n\nThese steps are strictly sequential — launch each only after the "
                    "previous completes. Do not parallelize.")

    # Shared rules + state-specific context (only for action intents)
    action_intent = False
    if intent in ("instructions", "proposal_request", "approval", "correction"):
        action_intent = True
    elif intent == "question" and execute_count > 0:
        action_intent = True

    if action_intent:
        context += (
            "\n\nAny architectural changes to any plan are a hard blocker — require user "
            "approval before proceeding.\n\nNever change the scope of the user's requirements "
            "without approval. No adding features, removing requirements, reinterpreting "
            "terminology, creating files outside the stated scope, or hacking infrastructure as a "
            "workaround. If scope needs to change, state what and why, then wait for approval "
            "before proceeding.\n\nWhen the user gives feedback on a decision, evaluate the options "
            "and present findings — never conclude with a decision. The user is the decision "
            "maker.\n\nZero-guess policy — every code assertion must be validated against the source "
            "before it is claimed. No statement about what code does, calls, returns, contains, or "
            "causes without reading it first. Applies to reports, summaries, answers, and proposals "
            "— not just edits. Pattern matching is not validation. The correct answer beats the "
            "quick answer — never trade rigor for speed."
        )
        context += ("\n\nDeliverable lands in this turn's final user-visible text. Not in "
                    "thinking, not pre-tool-call, not deferred.")
        if approach in ("subagents", "team"):
            context += (
                "\n\nWhen dispatching subagents: communicate WHY and WHAT only — not HOW (unless "
                "the HOW is a unique finding the subagent won't realistically discover reading the "
                "code). Do not pre-research, pre-read files, or run commands to \"prepare\" for a "
                "subagent. Do not include information the subagent can find in the codebase. The "
                "value of subagents is fresh, unbiased context — over-instruction destroys "
                "this.\n\nWhen told to do something N times in parallel, run all N in parallel — "
                "never serialize. Use a single message with multiple Agent tool calls."
            )

    if action_intent and new_state == "proposing":
        proposal_directive = (
            "This is a proposing-state turn. Load the /propose skill now and produce the proposal "
            "under its contract. Every proposal you emit on a proposing turn passes the /propose "
            "seven-failure self-check before sending: vacuous-proposal, capability-loss, "
            "worse-option-shipped, requirement-drop, contradiction-elision, mixed-layer-pcc, "
            "hedged-proposal. The skill is mandatory, not optional."
        )
        if current_notes:
            formatted_notes = "\n".join("- %s" % n for n in current_notes)
            if formatted_notes:
                proposal_directive += ("\n\nPast corrections from this session — do not "
                                       "re-violate:\n" + formatted_notes)
        context += "\n\n" + proposal_directive
        if "/propose" not in skills:
            skills = ("%s, /propose" % skills) if skills else "/propose"
    elif action_intent and new_state == "executing":
        execute_directive = (
            "This is an executing-state turn. Load the /execute skill now and work under its "
            "contract. You are implementing work the architect already approved; the moment it "
            "requires an architectural change — creating, renaming, moving, or deleting a file, "
            "public method, schema, or dependency, or introducing an unprecedented pattern — stop "
            "and escalate with /pcc instead of making the call. Research the codebase before "
            "editing; never change code you haven't read this turn."
        )
        context += "\n\n" + execute_directive
        if "/execute" not in skills:
            skills = ("%s, /execute" % skills) if skills else "/execute"
    elif action_intent and new_state == "auto":
        context += ("\n\nThis message contains mixed intents. Execute action items first, then "
                    "answer questions. The user expects actions completed before discussion. "
                    "Research the codebase before editing. Never change code you haven't read.")

    # Commit context
    if commit_requested:
        context += "\n\n" + COMMIT_DIRECTIVE

    # Recommended agents (injected when classifier identifies matching specialists)
    if recommended_agents:
        context += "\n\nRecommended agents for this task:\n" + recommended_agents

    # Prepend state change notifications
    if state_notifications:
        context = ("State changes (applied by classifier, no action needed):\n%s\n%s"
                   % (state_notifications, context))

    # Head-anchor restatement instruction (must be first for primacy)
    restatement = ("First line: tell the architect in your own words what you understood from %s. "
                   "Preserve every constraint they stated; add none. Then act." % restatement_target)
    if intent in ("instructions", "proposal_request"):
        restatement += " Execute detected /skills immediately after restating."
    context = restatement + "\n\n" + context

    emit_context(context)
    return 0


if __name__ == "__main__":
    sys.exit(main())
