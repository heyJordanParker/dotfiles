"""Honcho memory: every call the hooks make, in and out.

The plugin's own uploaders decided who spoke from which hook fired: anything
arriving on UserPromptSubmit was stored as the architect, so task notifications,
hook-injected blocks, and skill loads all became his speech, and the server
derived "jordan instructed…" from an agent's own words. Every path lives here
now, where `lib/transcript.py` can answer who actually spoke before anything is
sent, and the plugin is uninstalled.

Memory is one session per conversation, recording its repository, Agent, model
and Harness. The peers are the architect and one per Agent on one model
(`cto-claude-opus-5-5`). Each project, named by its `origin` remote, is a Honcho
scope, and a session joins its project's scope when it is created. One derivation
per message fills the speaker's own view and the scope's view of the speaker.
Deliberate saves go to session `general` of an own view. An Agent's replies are
derived too, steered by `LESSONS` to what should change its next run, and the
architect's by `PRINCIPLES` to how he works rather than what he asked for.

Stdlib only, so this speaks to the v3 REST API directly instead of through
`@honcho-ai/sdk`. Agents reach Honcho themselves through the official `honcho`
CLI. Reads `~/.honcho/config.json`, the file that CLI reads too.

Every failure is silent. A memory call is never worth blocking a turn over, and
the hooks that call this have nothing to say to the agent. A write answers False,
and a read answers None when Honcho did not answer and "" or [] when it holds
nothing.
"""

import getpass
import json
import os
import re
import subprocess
import urllib.error
import urllib.parse
import urllib.request

from lib import agent_memory, transcript
from lib.event import agent_name, field, is_subagent

API_VERSION = "v3"
CONFIG_PATH = os.path.expanduser("~/.honcho/config.json")

# Honcho caps a message at 25k characters. 24k leaves the same headroom the
# plugin left, so a long turn splits the same way on either writer.
MAX_MESSAGE = 24000

TIMEOUT = 10
STORE_TIMEOUT = 2


def config():
    """The committed server and workspace, speaking as the machine's login name unless it names a peer."""
    try:
        with open(CONFIG_PATH, encoding="utf-8") as fh:
            cfg = json.load(fh)
    except (OSError, ValueError):
        return {}
    cfg.setdefault("peerName", _sanitize(getpass.getuser()))
    return cfg


def enabled(cfg):
    """Whether memory is on at all — the one switch, governing every path."""
    return bool(cfg) and cfg.get("enabled") is not False


def _sanitize(name):
    return "".join(c if c.isalnum() or c in "-_" else "-" for c in name.lower())


def project_root(cwd):
    """The repository a directory belongs to, or the directory itself.

    `git rev-parse --git-common-dir` resolves a linked worktree to the main
    checkout's `.git`, so every worktree of a repo answers with the same root and
    shares one memory. It also collapses a subdirectory onto its repo, which the
    plugin's `basename(cwd)` never did — working in `packages/agents/hooks` minted
    a `jordan-hooks` session separate from `jordan-dotfiles`.
    """
    try:
        out = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=cwd,
                             capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        return cwd
    common = out.stdout.strip()
    if out.returncode != 0 or not common:
        return cwd
    if not os.path.isabs(common):
        common = os.path.join(cwd, common)
    # `<root>/.git` for a normal repo and for a worktree alike; a bare repo
    # answers with the repo directory itself, which is already the root.
    root = os.path.dirname(os.path.normpath(common))
    return root or cwd


def _origin_name(cwd):
    """The repository name its `origin` remote carries, or "" when it has none."""
    try:
        out = subprocess.run(["git", "remote", "get-url", "origin"], cwd=cwd,
                             capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        return ""
    url = out.stdout.strip().rstrip("/")
    if out.returncode != 0 or not url:
        return ""
    name = re.split(r"[/:]", url)[-1]
    return name[:-4] if name.endswith(".git") else name


def project_name(cwd):
    """The project a directory belongs to: its repository, named by its remote.

    It names the project's scope, which holds every conversation held in that
    repository, whoever holds it.

    Derived every time, with no per-directory override table. A stored mapping is
    a second source of truth for a name the repo already answers, and the one that
    accumulated here pinned a worktree to its own memory — splitting one project in
    half — and minted one per scratch directory besides.

    The `origin` remote names the repository, so a folder rename leaves its
    memory where it was; a repository with no remote falls back to its folder."""
    root = project_root(cwd)
    return _sanitize(_origin_name(root) or os.path.basename(root.rstrip("/")) or "root")


def peer_name(agent, model):
    """The peer an Agent runs as on one model, such as `cto-claude-opus-5-5`, or "".

    How an Agent works, and what it learns, changes with the model it runs on, so
    each pairing is its own peer and its views stay apart without any filter."""
    return _sanitize("%s-%s" % (agent, model)) if agent and model else ""


def conversation(event):
    """The Harness's own id for the conversation an event belongs to.

    A Claude Subagent is a conversation of its own, with its own transcript and
    model: its payload's `session_id` is the parent's, and `agent_id` is its own."""
    return field(event, "agent_id", "") or field(event, "session_id", "")


def event_model(event, recs):
    """The model a conversation runs on, or "".

    codex puts it on every Hook payload. Claude records it only on a reply, so a
    Claude conversation's first turn has none."""
    return field(event, "model", "") or transcript.model(recs)


def harness(event):
    """`codex` or `claude`: `turn_id` is codex's own payload field, and Claude sends none."""
    return "codex" if field(event, "turn_id", "") else "claude"


def chunks(text):
    """Text split under the message cap, preferring a newline then a space break."""
    out = []
    rest = text
    while len(rest) > MAX_MESSAGE:
        cut = rest.rfind("\n", 0, MAX_MESSAGE)
        if cut < MAX_MESSAGE // 4:
            cut = rest.rfind(" ", 0, MAX_MESSAGE)
        if cut < MAX_MESSAGE // 4:
            cut = MAX_MESSAGE
        out.append(rest[:cut])
        rest = rest[cut:].lstrip()
    if rest:
        out.append(rest)
    return out


# Derived from an Agent's reply by default, Honcho records what the Agent did:
# the old per-Agent collections held 50 of 50 commands and file writes. With
# these added instructions, a live test reply listing a fix, a mistake and a
# test count yielded the mistake's lesson and a tool quirk, and nothing else.
LESSONS = ("The target peer is an AI coding agent. Write a conclusion only for what it observed that "
           "changes how it works next time: a mistake it made and its cause, how a tool, system, or "
           "environment behaved, an approach that worked. A proposal, plan, or recommendation is not "
           "an observation, because nothing has shown it works yet. Write no conclusion for one, for "
           "what it did, changed, or ran, or for facts about the code, because the transcript and "
           "the repository already hold them.\n"
           "Example: <message peer=\"builder-model-x\" target=\"true\">The migration passed its tests "
           "and failed on live data, because the fixtures had no null rows. I added null rows and "
           "reran it; all 40 tests pass. I propose a nightly run against a copy of live data."
           "</message> → \"builder-model-x learned that fixtures without null rows let a migration "
           "pass tests that live data broke\"; the rerun, the test count, and the proposed nightly "
           "run get no conclusion.")

# Derived from the architect's messages by default, Honcho records every request
# he makes: 23 of his messages gave 54 lines, mostly one-off tasks. These
# instructions gave 19, nearly all principles and corrections with their reasons.
PRINCIPLES = ("The target peer directs AI coding agents, so \"you\" in their messages means the agent. "
              "Write a conclusion only for what will still matter on their next task: a principle "
              "they work from and why, what they value or reject, how they judge work, a correction "
              "they made and why, an approach they confirmed. Write no conclusion for a request for "
              "one task, a report of one bug, or a question about status.\n"
              "Example: <message peer=\"dana\" target=\"true\">keep it simple, every abstraction has "
              "to earn its place</message> → \"dana favors the simplest design that works and "
              "expects every abstraction to justify itself\"\n"
              "Example: <message peer=\"dana\" target=\"true\">the export button is broken, fix "
              "it</message> → no conclusion; it asks for one task.")


def post(cfg, session, said, timeout=None):
    """Store ordered peer-and-text pairs in a session. True when the API took them."""
    if not session:
        return False
    messages = []
    for peer, text in said:
        if not peer or not text.strip():
            continue
        instructions = PRINCIPLES if peer == cfg.get("peerName") else LESSONS
        messages.extend({"peer_id": peer, "content": chunk,
                         "configuration": {"reasoning": {"custom_instructions": instructions}}}
                        for chunk in chunks(text))
    if not messages:
        return False
    return _request(cfg, "POST", "sessions/%s/messages" % urllib.parse.quote(str(session), safe=""),
                    body={"messages": messages}, timeout=timeout) is not None


def members(architect, agent_peer):
    """Who is in a conversation: both are learned about, and neither observes the other.

    Honcho writes each speaker's lines into its own view and into the view of
    every scope the session belongs to, so no member needs to observe."""
    out = {}
    if architect:
        out[architect] = {"observe_me": True, "observe_others": False}
    if agent_peer:
        out[agent_peer] = {"observe_me": True, "observe_others": False}
    return out


def open_session(cfg, session, peers, metadata, scope, timeout=None):
    """Make `session` exist with exactly `peers` as members. True when Honcho took every request.

    A session joins `scope` only on the request that creates it. Joining a session
    that already has messages queues a backfill that copies all of them again.
    Setting the members rather than adding them retires a model's peer the moment
    the conversation moves to another model, and leaves the scope membership alone."""
    if not session or not peers:
        return False
    body = {"id": session, "metadata": {key: value for key, value in metadata.items() if value}}
    status, _ = _call(cfg, "POST", "sessions", body=body, timeout=timeout)
    if status is None:
        return False
    if status == 201 and _request(cfg, "POST", "sessions", body={"id": session, "scopes": [scope]},
                                  timeout=timeout) is None:
        return False
    return _request(cfg, "PUT", "sessions/%s/peers" % urllib.parse.quote(str(session), safe=""),
                    body=peers, timeout=timeout) is not None


def store(cfg, event, recs, agent, architect_said, replies):
    """Store one complete turn in its conversation. True when Honcho took it.

    The Agent-on-model joins its conversation once the model is known, which on
    Claude is from the Agent's first reply. Until then the architect's words reach
    his own view and the project scope's.

    Its requests share the calling Hook's 10-second limit, so each gets
    `STORE_TIMEOUT`: a stalled server loses one message instead of showing every
    running session a Hook timeout."""
    model = event_model(event, recs)
    agent_peer = peer_name(agent, model)
    architect = "" if is_subagent(event) else cfg.get("peerName", "")
    project = project_name(field(event, "cwd", "") or os.getcwd())
    session = conversation(event)
    said = ([(architect, text) for text in architect_said]
            + [(agent_peer, replies)])
    if not any(peer and text.strip() for peer, text in said):
        return False
    metadata = {"repository": project, "agent": agent, "model": model, "harness": harness(event)}
    if not open_session(cfg, session, members(architect, agent_peer), metadata, project,
                        timeout=STORE_TIMEOUT):
        return False
    return post(cfg, session, said, timeout=STORE_TIMEOUT)


def _request(cfg, method, route, body=None, query=None, timeout=None):
    """Run one Honcho request, returning decoded JSON or None on failure."""
    return _call(cfg, method, route, body=body, query=query, timeout=timeout)[1]


def _call(cfg, method, route, body=None, query=None, timeout=None):
    """Run one Honcho request, returning its status and decoded JSON, or (None, None) on failure."""
    base = (cfg.get("environmentUrl") or "").rstrip("/")
    workspace = cfg.get("workspace")
    if not base or not workspace:
        return None, None

    url = "%s/%s/workspaces/%s/%s" % (base, API_VERSION, workspace, route.lstrip("/"))
    if query:
        url += "?" + urllib.parse.urlencode(query)
    data = json.dumps(body).encode("utf-8") if body is not None else None
    request = urllib.request.Request(url, data=data, method=method)
    if data is not None:
        request.add_header("Content-Type", "application/json")
    api_key = cfg.get("apiKey")
    if api_key:
        request.add_header("Authorization", "Bearer %s" % api_key)

    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT if timeout is None else timeout) as response:
            if not 200 <= response.status < 300:
                return None, None
            data = response.read().decode("utf-8")
            return response.status, json.loads(data) if data else {}
    except (urllib.error.URLError, OSError, ValueError, UnicodeDecodeError):
        return None, None


# Ten conclusions measured 1,915 characters. Two peers' worth and their general
# lines fit one turn's context whole.
MAX_CONCLUSIONS = 10

# How far a line may sit from a turn's words and still be read. Measured on the
# architect's own phrasing: at 0.7 "is this a good memory system or not? check"
# found 3 memory lines and a request nothing stored answers found none. At 0.6 the
# first found nothing, and at 0.8 the second found 10 unrelated lines.
DISTANCE = 0.7


def search(cfg, peer, project, query, timeout=None):
    """The project's lines about `peer` that match `query`, nearest first, [] when none match, None on failure.

    Honcho's plain search over the scope's view. Its representation read fills
    every slot the hits leave with the newest lines, so a turn nothing stored
    matched got ten memories about other work."""
    response = _request(cfg, "POST", "conclusions/query", body={
        "query": query, "top_k": MAX_CONCLUSIONS, "distance": DISTANCE,
        "filters": {"observer": "scope." + project, "observed": peer}}, timeout=timeout)
    return _dated(response) if isinstance(response, list) else None


def card(cfg, project, peer, timeout=None):
    """The project scope's card of `peer`: its standing facts, [] when none, None on failure."""
    response = _request(cfg, "GET", "peers/%s/card" % urllib.parse.quote("scope." + project, safe=""),
                        query={"target": peer}, timeout=timeout)
    if not isinstance(response, dict):
        return None
    return response.get("peer_card") or []


def patterns(cfg, peer, project, timeout=None):
    """The patterns Honcho's dream drew about `peer` in the project, newest first, [] when none, None on failure."""
    page = _request(cfg, "POST", "conclusions/list", body={"filters": {
        "observer_id": "scope." + project, "observed_id": peer, "level": "inductive"}},
        query={"size": MAX_CONCLUSIONS}, timeout=timeout)
    return _dated(page["items"]) if isinstance(page, dict) else None


def _dated(items):
    return ["[%s] %s" % (item["created_at"][:10], item["content"].strip()) for item in items]


GENERAL = "general"


def general(cfg, peer, query="", timeout=None):
    """The lines of a peer's own view that hold in every project, [] when none match, None on failure.

    An own view collects every project's lessons, so only deliberate saves and
    patterns whose evidence spans two projects are read outside their project.
    With no query to search on, the newest deliberate saves stand in."""
    if not peer:
        return []
    if not query:
        page = _request(cfg, "POST", "conclusions/list", body={"filters": {
            "observer_id": peer, "observed_id": peer, "session_id": GENERAL}},
            query={"size": MAX_CONCLUSIONS}, timeout=timeout)
        return _dated(page["items"]) if isinstance(page, dict) else None
    response = _request(cfg, "POST", "conclusions/query", body={
        "query": query, "top_k": 5, "distance": DISTANCE,
        "filters": {"observer": peer, "observed": peer, "OR": [
            {"session_id": GENERAL}, {"level": {"in": ["deductive", "inductive"]}}]},
    }, timeout=timeout)
    if not isinstance(response, list):
        return None
    sources = _by_id(cfg, "conclusions/list",
                     [sid for item in response if item["session_id"] != GENERAL
                      for sid in item["source_ids"]], timeout)
    sessions = _by_id(cfg, "sessions/list",
                      sorted({source["session_id"] for source in sources or [] if source["session_id"]}),
                      timeout)
    if sources is None or sessions is None:
        return None
    session_of = {source["id"]: source["session_id"] for source in sources}
    project_of = {session["id"]: session["metadata"].get("repository") for session in sessions}

    def projects(item):
        return {project_of.get(session_of.get(sid)) for sid in item["source_ids"]} - {None}

    return _dated([item for item in response
                   if item["session_id"] == GENERAL or len(projects(item)) >= 2])


def _by_id(cfg, route, ids, timeout):
    if not ids:
        return []
    response = _request(cfg, "POST", route, body={"filters": {"id": ids}}, query={"size": 100},
                        timeout=timeout)
    return response["items"] if isinstance(response, dict) else None


def running_agent():
    """The agent this process is running as, or "".

    A Claude session carries the name it was started as. Inside a Claude subagent
    `CLAUDE_CODE_AGENT` still holds the dispatching agent, which is why
    `memory_agent` reads the event's own agent first.
    """
    return os.environ.get("CLAUDE_CODE_AGENT", "")


def memory_agent(event):
    """The agent whose memory an event belongs to, on either harness, or "".

    The event's agent when it is one of ours. Otherwise the agent the session runs
    as: a Claude fork and a Subagent codex starts under its own role name report
    names that are not agents of ours, and they work for the agent that started
    them."""
    named = agent_name(event)
    if named and os.path.isfile(agent_memory.definition_path(named)):
        return named
    return running_agent()
