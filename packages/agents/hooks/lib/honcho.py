"""Honcho memory: the whole path in and out, plus the `honcho` command.

The plugin's own uploaders decided who spoke from which hook fired: anything
arriving on UserPromptSubmit was stored as the architect, so task notifications,
hook-injected blocks, and skill loads all became his speech, and the server
derived "jordan instructed…" from an agent's own words. Every path lives here
now, where `lib/transcript.py` can answer who actually spoke before anything is
sent, and the plugin is uninstalled.

Memory is stored per peer, one peer per human and one per agent, all inside one
session per repository. Nobody observes anybody: each peer's collection holds
Honcho's own conclusions about that peer's messages, so nothing is derived twice
and no conclusion exists in two places. An agent's collection therefore fills
with what it said; `remember` is how something it was told gets in.

Stdlib only, so this speaks to the v3 REST API directly instead of through
`@honcho-ai/sdk`, and `packages/bin/honcho` is a two-line wrapper around `main`
the way `codex-run` wraps `lib/codex_run.py`. Reads `~/.honcho/config.json`.

Every hook-facing failure is silent. A memory write is never worth blocking a
turn over, and the hooks that call this have nothing to say to the agent. A write
answers False, a list answers [], and only `context` distinguishes its failure
from its empty answer, because its caller acts differently on each. The command
line reports its failures, because a human ran it.
"""

import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request

API_VERSION = "v3"
CONFIG_PATH = os.path.expanduser("~/.honcho/config.json")

# Honcho caps a message at 25k characters. 24k leaves the same headroom the
# plugin left, so a long turn splits the same way on either writer.
MAX_MESSAGE = 24000

TIMEOUT = 10


def config():
    try:
        with open(CONFIG_PATH, encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, ValueError):
        return {}


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


def session_name(cwd):
    """The session a directory writes to: its repository.

    Derived every time, with no per-directory override table. A stored mapping is
    a second source of truth for a name the repo already answers, and the one that
    accumulated here pinned a worktree to its own session — splitting one project's
    memory in half — and minted a session per scratch directory besides.

    No peer prefix: a session holds many peers, so one repository is one session
    that every human and every agent working it writes into. Prefixing by peer
    would split the same project per person the moment a second human arrives.

    The server creates a session on first write, so a repo that has never been
    seen needs no registration."""
    return _sanitize(os.path.basename(project_root(cwd).rstrip("/")) or "root")


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


def post(cfg, session, peer, text, metadata=None, timeout=None):
    """Store text in a session as one peer's speech. True when the API took it.

    The peer is the caller's decision and never inferred here — that inference is
    the defect this module exists to remove."""
    if not session or not peer or not text:
        return False
    messages = [{"peer_id": peer, "content": chunk, "metadata": dict(metadata or {})}
                for chunk in chunks(text)]
    return _request(cfg, "POST", "sessions/%s/messages" % urllib.parse.quote(str(session), safe=""),
                    body={"messages": messages}, timeout=timeout) is not None


def _request(cfg, method, route, body=None, query=None, timeout=None):
    """Run one Honcho request, returning decoded JSON or None on failure."""
    base = ((cfg.get("endpoint") or {}).get("baseUrl") or "").rstrip("/")
    workspace = cfg.get("workspace")
    if not base or not workspace:
        return None

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
                return None
            data = response.read().decode("utf-8")
            return json.loads(data) if data else {}
    except (urllib.error.URLError, OSError, ValueError, UnicodeDecodeError):
        return None


def context(cfg, peer, query="", timeout=None):
    """Relevant conclusion text for one peer, [] when it has none, None on failure.

    The three are different answers and a caller acts differently on each: a peer
    Honcho knows nothing about is a fact, a server that did not answer is not.

    `include_most_frequent` carries the conclusions that hold regardless of what
    the turn is about. With it off, a turn whose words match nothing semantically
    got an empty block instead of a profile."""
    if not peer:
        return []
    response = _request(cfg, "GET", "peers/%s/context" % urllib.parse.quote(str(peer), safe=""),
                        query={
                            "search_query": query,
                            "search_top_k": 10,
                            "search_max_distance": 0.6,
                            "max_conclusions": 15,
                            "include_most_frequent": "true",
                        }, timeout=timeout)
    if not isinstance(response, dict):
        return None
    representation = response.get("representation")
    if not isinstance(representation, str):
        return None
    return [re.sub(r"^- ", "", re.sub(r"^\[.*?\]\s*", "", line)).strip()
            for line in representation.splitlines()
            if line.strip() and not line.startswith("#")]


CACHE_DIR = os.path.expanduser("~/.honcho/cache")


def _cache_path(cfg, peer):
    # Keyed by workspace as well as peer: the same peer name lives in every
    # workspace, and a fallback served across that line would put one workspace's
    # conclusions in front of another's turn.
    return os.path.join(CACHE_DIR, "%s.%s.json" % (_sanitize(str(cfg.get("workspace") or "none")),
                                                   _sanitize(str(peer))))


def remembered_context(cfg, peer, query="", timeout=None):
    """A peer's conclusions, falling back to the last set that arrived.

    The retrieval is one network call in front of every turn. When it fails or
    times out, returning nothing makes that turn memory-blind and nothing says
    so; the last successful answer is stale but true, and the turn reads as a
    turn rather than as a fresh start. Every success replaces the fallback.

    Only a failure falls back. An answered "this peer has nothing" is the truth
    about a peer, and treating it as a failure kept replaying a stale set at a
    fresh agent forever — its collection would have had to out-argue a cache that
    was never allowed to empty.
    """
    lines = context(cfg, peer, query=query, timeout=timeout)
    if lines is not None:
        try:
            os.makedirs(CACHE_DIR, exist_ok=True)
            with open(_cache_path(cfg, peer), "w", encoding="utf-8") as fh:
                json.dump(lines, fh)
        except OSError:
            pass
        return lines
    try:
        with open(_cache_path(cfg, peer), encoding="utf-8") as fh:
            stale = json.load(fh)
    except (OSError, ValueError):
        return []
    return stale if isinstance(stale, list) else []


def card(cfg, peer, timeout=None):
    """The peer card: the standing lines about a peer, independent of any query.

    The one part of the plugin's block that never depended on the turn's words
    matching a conclusion semantically."""
    if not peer:
        return []
    response = _request(cfg, "GET", "peers/%s/card" % urllib.parse.quote(str(peer), safe=""),
                        timeout=timeout)
    lines = response.get("peer_card") if isinstance(response, dict) else None
    return [str(line).strip() for line in lines if str(line).strip()] if lines else []


def search(cfg, query, peer=None):
    """Messages matching `query`, scoped to a peer or to the whole workspace.

    Conclusions are what a turn is injected with; the messages behind them are
    reachable only here. "What did we decide about X" is a message question."""
    if not query:
        return []
    # `_request` already prefixes `workspaces/<workspace>`; naming it again here
    # built `/workspaces/<w>/<w>/search`, which 404s into an empty list that reads
    # exactly like "no matches".
    route = "peers/%s/search" % urllib.parse.quote(str(peer), safe="") if peer else "search"
    response = _request(cfg, "POST", route, body={"query": query, "limit": 10})
    items = response.get("items") if isinstance(response, dict) else response
    return items if isinstance(items, list) else []


def ask(cfg, peer, question):
    """Honcho's reasoning over everything it knows about a peer, None on failure.

    The dialectic endpoint: it searches and reasons rather than returning stored
    lines, which is the one read no amount of conclusion retrieval substitutes for.
    A human waits on this one, which is what buys it the 120s ceiling.

    Failure and an empty answer are told apart the way `context` tells them apart,
    and for the same reason: this endpoint answered 500 for every peer while the
    rest of the API was healthy, and "no answer" read as "memory knows nothing
    about him"."""
    if not peer or not question:
        return None
    response = _request(cfg, "POST", "peers/%s/chat" % urllib.parse.quote(str(peer), safe=""),
                        body={"query": question, "stream": False, "reasoning_level": "low"},
                        timeout=120)
    if not isinstance(response, dict):
        return None
    content = response.get("content")
    return content if isinstance(content, str) else ""


def conclusions(cfg, filters=None):
    """The first page of raw conclusion objects matching filters, or [] on failure."""
    response = _request(cfg, "POST", "conclusions/list", body={"filters": filters},
                        query={"page": 1, "size": 50})
    if isinstance(response, list):
        return response
    if isinstance(response, dict) and isinstance(response.get("items"), list):
        return response["items"]
    return []


def create_conclusion(cfg, observer, observed, content):
    """Create one conclusion. Return False when Honcho cannot accept it.

    No session: the collection is the key a read asks for, and naming a session
    the messages have not created yet is a 404 rather than a placement."""
    if not observer or not observed or not content:
        return False
    response = _request(cfg, "POST", "conclusions", body={"conclusions": [{
        "observer_id": observer,
        "observed_id": observed,
        "content": content,
        "session_id": None,
    }]})
    return response is not None


def delete_conclusion(cfg, conclusion_id):
    """Delete one conclusion. Return False when Honcho cannot accept it."""
    if not conclusion_id:
        return False
    response = _request(cfg, "DELETE", "conclusions/%s" %
                        urllib.parse.quote(str(conclusion_id), safe=""))
    return response is not None


# --- command line ---------------------------------------------------------------

USAGE = """honcho — read and write Honcho memory

  honcho remember [--as <agent>] <text>   keep one thing in an agent's collection
  honcho context <peer> [query]           the conclusions the server holds now
  honcho ask <peer> <question>            reason over everything known about a peer
  honcho search <query> [--peer <p>]      the stored messages behind the conclusions
  honcho list [peer]                      the first page of raw conclusions
  honcho forget <id>                      delete one conclusion

`remember` names the running agent itself; `--as` is for writing into another
agent's collection, which is the architect's call to make.

`context` returns stored conclusions; `ask` reasons over the messages behind them
and answers in prose; `search` returns the messages themselves. A question about
what was decided is a `search` or an `ask`, never a `context`."""

NO_AGENT = """honcho: nothing here names which agent is running, so a write has
no collection to land in. Name one with `--as <agent>`.
"""

def running_agent():
    """The agent this process is running as, or "".

    A codex run carries its own definition path, exported by its launcher, and a
    Claude session carries the name it was started as. Inside a Claude subagent
    neither is right — `CLAUDE_CODE_AGENT` still holds the dispatching agent —
    which is why `name_memory_caller.py` writes `--as` into the command there
    before it reaches this function.
    """
    path = os.environ.get("CODEX_RUN_AGENT_FILE", "")
    if path.endswith(".md"):
        return os.path.basename(path)[:-3]
    return os.environ.get("CLAUDE_CODE_AGENT", "")


def main(argv):
    cfg = config()
    if not enabled(cfg):
        sys.stderr.write("honcho memory is disabled in %s\n" % CONFIG_PATH)
        return 1

    command = argv[0] if argv else ""
    rest = argv[1:]

    if command == "remember" and rest:
        named = ""
        if rest[0] == "--as":
            # Without both a name and something to keep, the flag and its value
            # would otherwise be stored as the text itself.
            if len(rest) < 3:
                sys.stderr.write(USAGE + "\n")
                return 1
            named, rest = rest[1], rest[2:]
        text = " ".join(rest)
        agent = named or running_agent()
        if not agent:
            sys.stderr.write(NO_AGENT)
            return 1
        # Observer and observed are both the agent: the subject is its own
        # behaviour, and no second party's view of it differs.
        if not create_conclusion(cfg, agent, agent, text):
            sys.stderr.write("honcho refused the write\n")
            return 1
        sys.stdout.write("remembered for %s\n" % agent)
        return 0

    if command == "context" and rest:
        lines = context(cfg, rest[0], query=" ".join(rest[1:]))
        if lines is None:
            sys.stderr.write("honcho did not answer for %s\n" % rest[0])
            return 1
        for line in card(cfg, rest[0]) + lines:
            sys.stdout.write("- %s\n" % line)
        return 0

    if command == "ask" and len(rest) > 1:
        answer = ask(cfg, rest[0], " ".join(rest[1:]))
        if answer is None:
            sys.stderr.write("honcho did not answer. The rest of the API may still be up: "
                             "this endpoint reasons with a model and fails on its own.\n")
            return 1
        if not answer:
            sys.stderr.write("honcho knows nothing about %s that answers this\n" % rest[0])
            return 1
        sys.stdout.write(answer + "\n")
        return 0

    if command == "search" and rest:
        peer = ""
        if "--peer" in rest:
            at = rest.index("--peer")
            if at + 1 >= len(rest):
                sys.stderr.write(USAGE + "\n")
                return 1
            peer, rest = rest[at + 1], rest[:at] + rest[at + 2:]
        for item in search(cfg, " ".join(rest), peer=peer or None):
            sys.stdout.write("%s  %s  %s\n" % (
                (item.get("created_at") or "")[:19], item.get("peer_id", ""),
                " ".join((item.get("content") or "").split())[:200]))
        return 0

    if command == "list":
        filters = {"observed_id": rest[0]} if rest else None
        for item in conclusions(cfg, filters=filters):
            sys.stdout.write("%s  (%s, %s)  %s\n" % (
                item.get("id", ""), item.get("observer_id", ""),
                item.get("observed_id", ""), item.get("content", "")))
        return 0

    if command == "forget" and rest:
        if not delete_conclusion(cfg, rest[0]):
            sys.stderr.write("honcho refused the delete\n")
            return 1
        sys.stdout.write("deleted %s\n" % rest[0])
        return 0

    sys.stderr.write(USAGE + "\n")
    return 1
