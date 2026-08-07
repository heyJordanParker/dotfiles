"""codex_run — run codex as one of our named agents, mechanically.

The orchestrating agent used to hand-assemble the whole codex invocation every
time it dispatched codex as one of our agents: the flags, a `/tmp` output path,
an inline JSONL parser, a failure grep. That assembly was duplicated across the
codex skills and needed no judgment. This module owns all of it, so the agent
only writes the task prompt.

`codex-run @<agent> "<prompt>"` resolves `@<agent>` to that agent's
frontmatter-stripped instruction body (`<roster>/<name>.prompt.md`, the same
artifact that boots codex as the CTO) and runs codex under it. An unknown agent
exits non-zero listing the available agents.

The transport is `codex app-server`, one process per run, owned by this runner
and spoken to over newline-delimited JSON-RPC on its stdio. It replaced a
one-shot `codex exec --json` subprocess, which gave the wrapper no way to say
anything to codex once the run began: there was no cancellation, so killing a
run killed codex mid-apply_patch and left half-edited files, and there was no
liveness check, so a wedged codex hung until the harness ceiling. A live
connection has a control channel (`turn/interrupt`) and a message stream whose
silence the idle deadlines measure, so both are now reachable.

Everything an agent declares rides inline on `thread/start` and `thread/resume`:
the instructions as `baseInstructions`, the model and effort as their own
fields, and `memory: none` as a `config` object switching off both of codex's
memory providers. Nothing is written to disk for codex to read, and nothing is
inherited from the interactive `config.toml`, so a run states in one request
exactly what governs it — including its MCP servers, which a run that named
none simply did not have.

Every run writes a job record — a JSON file beside the answer and the event
stream, sharing their collision-free stem, which is therefore the job id. The
record holds the agent, codex's thread id and rollout path, the model, the
effort, the status and phase, the pids, the output paths and the timestamps.
That record is what makes a continuation cheap: `codex-run resume <job> "<msg>"`
reads the founding agent off a field this runner wrote, where the old wrapper
had to work it out by globbing codex's session archive for the thread's rollout
and matching the recorded instructions back against every agent's prompt.md.
Identity is now recorded, not reconstructed.

A resume whose thread codex no longer holds starts a fresh thread from the
record's own settings and reruns the turn once, saying plainly in the output
that it did not resume — the alternative, a silent fresh thread, reads exactly
like a continuation and is not one.

Our shared Python guards govern the run, not codex's sandbox — the architect
does not want codex sandboxes — so every run asks for `danger-full-access` with
approvals `never`. Our hooks are our own vetted sources, so every run also sets
`bypass_hook_trust`, which runs them without codex's per-command trust gate.

Output lands in the session's own directory via the session-state helper, never
`/tmp`, with the same no-session fallback the model runner uses.

The result is printed to stdout, ready to read with no downstream parsing: the
final answer, a `--- codex-run ---` delimiter, then the status, the agent, the
model, the job id (for resume), codex's thread id, and the on-disk output and
events paths. Nothing the caller needs goes to stderr or requires a pipe — the
wrapper is the whole interface.

Alongside the run commands there is a job surface over those records: `status`
lists this session's jobs (`--all` scans sibling sessions), `result` prints an
answer, `log` renders an event stream as activity, `events` prints it raw,
`history` names codex's rollout and the Claude transcript, `cancel` interrupts a
turn and then kills the process tree, and `watch` tails a lifecycle feed.

That feed carries lifecycle only — started and the terminal line — because it
is built to be read by Claude Code's Monitor,
which allows ten events then one per two seconds and dies after thirty seconds
of continuous suppression. A single run in this repository emits sixty to a
hundred protocol events, around nine in ten of them command executions, so
forwarding events would kill the monitor within the first turn. Two lines per
job is the whole budget, and `status --all` is the cross-session view.

Stdlib only, matching the other hooks. Run via the `codex-run` launcher, which
exec's `main()`.
"""

import fcntl
import json
import os
import shlex
import signal
import subprocess
import sys
import threading
import time
from glob import glob

from lib import agent_memory, session_state

AGENTS_DIR = os.path.expanduser("~/.agents/agents")

# Model and effort default here rather than being inherited from config.toml:
# that file configures the interactive session, and an agent run is a different
# job — a scoped task dispatched by a coordinator, not an open-ended session.
# Defaulting means the agents cannot silently drift when the interactive default
# changes. An agent overrides either for itself in its frontmatter: `codex-model`
# for the model, the codex-side counterpart of `model`, which names a Claude model
# and so cannot serve here; and `effort`, which is one field for both harnesses,
# because the five words Claude's key takes are the five codex takes.
_MODEL = "gpt-5.6-terra"
_EFFORT = "medium"

# What an agent may declare: the five levels `claude --effort` lists, each of
# which codex also accepts verbatim, so a declaration crosses to either harness
# untranslated. `xhigh` and `max` are distinct levels on both — translating one
# onto the other quietly ran an agent at a tier it did not ask for, and omitting
# `xhigh` rejected the tier most likely to be declared.
_EFFORTS = ("low", "medium", "high", "xhigh", "max")

# Delimits the answer from the metadata trailer on stdout, so the result reads
# cleanly with no downstream parsing: everything above the line is codex's answer,
# everything below is status / agent / model / job / thread / output / events.
_TRAILER = "--- codex-run ---"


# --- the agent roster ---------------------------------------------------------------

def _roster_dirs():
    """The agents directories `@<agent>` resolves against, active root first.

    A profile is its own config root with its own roster, so an agent that lives
    only in a profile is a real agent while that profile is active and has to be
    runnable here. The active root's agents/ is read first — the same root, by
    the same rule, that the Claude-side gate reads a declaration from — so a name
    held by both a profile and the shared roster runs as the profile's, never the
    other way round. The shared roster follows rather than being replaced, so
    entering a profile adds a roster instead of losing one.

    The default root's agents/ is a symlink to the shared roster, so the two
    entries collapse to one; deduplication is on realpath, which catches that and
    any other aliasing between the roots."""
    root = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    dirs, seen = [], set()
    for candidate in (os.path.join(root, "agents"), AGENTS_DIR):
        key = os.path.realpath(candidate)
        if key not in seen:
            seen.add(key)
            dirs.append(candidate)
    return dirs


def _available_agents():
    """The agent names runnable as @<agent> — one per <name>.prompt.md, across
    every roster, with the first roster holding a name owning it."""
    names, seen = [], set()
    for directory in _roster_dirs():
        if not os.path.isdir(directory):
            continue
        for entry in sorted(os.listdir(directory)):
            if entry.endswith(".prompt.md"):
                name = entry[: -len(".prompt.md")]
                if name not in seen:
                    seen.add(name)
                    names.append(name)
    return sorted(names)


def _agent_dir(name):
    """The roster directory that owns `name`, or None when no roster holds it."""
    for directory in _roster_dirs():
        if os.path.isfile(os.path.join(directory, name + ".prompt.md")):
            return directory
    return None


def _definition_path(name):
    """`<name>.md` in the roster that owns the agent, or None when none does.

    Every declaration is read from here, so all of them come from the roster that
    also supplied the instructions — a profile's copy of an agent runs on the
    profile's declarations, never a mix of two rosters'."""
    directory = _agent_dir(name)
    return None if directory is None else os.path.join(directory, name + ".md")


def _declares_blank_memory(name):
    """Whether `<name>.md` declares `memory: none`.

    The same declaration the Claude-side gate reads, through the same parser."""
    path = _definition_path(name)
    return False if path is None else agent_memory.denies_memory(path)


def _declaration(name, key):
    """What `<name>.md` declares for `key`, or None when it declares nothing."""
    path = _definition_path(name)
    return None if path is None else agent_memory.declaration(path, key)


def _codex_model(name):
    """The codex model `name` runs on: its `codex-model` declaration, or _MODEL.

    The value is passed to codex unvalidated: codex owns which model names exist,
    an allowlist here would go stale every release, and a name codex rejects fails
    the run loudly with its own error already surfaced."""
    return _declaration(name, "codex-model") or _MODEL


def _codex_effort(name):
    """The reasoning effort `name` runs at: `codex-effort`, then `effort`, then _EFFORT.

    `effort` is one field across both harnesses — Claude reads the declaration
    natively and codex takes the same word verbatim, so declaring `high` cannot
    mean high on one harness and something else on the other. `codex-effort`
    sits above it for the codex run alone, the effort counterpart of
    `codex-model`, so one agent can run the two harnesses at different depths.
    Unlike the model, the value is checked: the vocabulary is closed and shared,
    so a word outside it is a typo in the definition rather than a level either
    harness has."""
    declared = _declaration(name, "codex-effort") or _declaration(name, "effort")
    if not declared:
        # A key with no value is an undeclared key, the same reading `codex-model`
        # gives it — two adjacent declarations of the same shape must not resolve
        # a blank in opposite directions.
        return _EFFORT
    if declared not in _EFFORTS:
        raise ValueError("agent %r declares unknown effort %r; valid: %s"
                         % (name, declared, ", ".join(_EFFORTS)))
    return declared


# The harness declaration, read as the counterpart of the Claude-side gate. An
# absent key and `all` both permit this run; anything else does not, including a
# value that is not a harness at all. The comparison is exact for the reason the
# model comparison is: this is an allowlist, so lowering it would widen permission
# rather than a denial.
_HERE = ("all", "codex")


def _resolve_agent(token):
    """`@<agent>` → the absolute path to its prompt.md, or None if unknown.

    The runnable set is exactly the named-agent allowlist (`_available_agents`),
    so the name must be one of those exact names — a name carrying path segments
    (`@../../something`) is rejected as unknown rather than joined onto a roster
    dir, which would let it reach an instruction file outside the named set. The
    allowlist lists bare filenames only, so no name in it can carry a segment."""
    if not token.startswith("@"):
        return None
    name = token[1:]
    if name not in _available_agents():
        return None
    directory = _agent_dir(name)
    return None if directory is None else os.path.join(directory, name + ".prompt.md")


# --- the app-server transport -------------------------------------------------------

_CLIENT = {"name": "codex-run", "title": "codex-run", "version": "1"}

# Declined at the handshake rather than filtered after arrival: the deltas are
# per-token, and a run that streams them writes tens of thousands of lines into
# the event file for an answer the item/completed already carries whole.
_DECLINED = ["item/agentMessage/delta", "item/reasoning/summaryTextDelta",
             "item/reasoning/summaryPartAdded", "item/reasoning/textDelta"]

# Our hooks are our own vetted sources, so the run does not need codex's
# per-command trust gate. This is the app-server spelling of the
# `--dangerously-bypass-hook-trust` flag the exec transport passed.
#
# mcp_servers rides here because the app-server inherits nothing from
# config.toml — an agent run that does not name a server has none, which is why
# a codex-run agent saw no MCP tools at all. context7 is named because the
# researcher runs on codex and current library documentation is its whole job.
_CONFIG = {"bypass_hook_trust": True,
            "mcp_servers": {"context7": {
                "command": "/Users/jordan/.bun/bin/bunx",
                "args": ["-y", "@upstash/context7-mcp"],
                "startup_timeout_sec": 120}}}

# An agent declaring `memory: none` runs with every memory provider off, not just
# one. Honcho is reached through the `honcho` command, which block_memory_access
# refuses for a blank-declared agent on either harness. codex's own [memories] is
# the second provider and needs no tool call at all, because it injects a Memory
# section and MEMORY_SUMMARY straight into the run — leaving it on let a
# blank-declared agent read Memory with Honcho already out of reach.
# `generate_memories` covers the write direction too, so a one-shot run cannot
# deposit anything for a later run to read.
_NO_MEMORY_CONFIG = {"memories": {"use_memories": False, "generate_memories": False}}

# A request codex has not answered in this long is a wedge, not slow work: every
# one of these is a handshake or a thread call, none of which waits on a model.
_REQUEST_LIMIT = 180

# Silence this long is the liveness check the exec transport had no equivalent
# for. It is deliberately far above a long reasoning pause — deltas are declined,
# so a max-effort turn can legitimately say nothing for minutes — and is there to
# end a codex that has stopped rather than one that is thinking.
# A turn can reason for several minutes before opening its first item.  That is
# not the same failure as codex going quiet after it has returned a tool result:
# the latter has a much tighter deadline, while this remains the backstop for a
# wholly silent run.
_POST_TOOL_IDLE_LIMIT = 600
_IDLE_LIMIT = 1200

# How long a cancelled turn is given to unwind after turn/interrupt before the
# process tree goes. The point of the interrupt is that codex finishes the file
# write it is inside; that takes seconds, not minutes.
_CANCEL_GRACE = 30


class _ServerError(Exception):
    pass


class _Server:
    """One `codex app-server` process, spoken to over JSON-RPC on its stdio.

    Both pipes are drained on their own threads. Reading stdout to exhaustion
    before touching stderr would deadlock on a run that writes enough stderr to
    fill its pipe buffer: codex blocks writing stderr, stops producing stdout,
    and the runner blocks reading stdout that never comes.

    The process is started in its own session, so its pid is also its process
    group id and `terminate` reaches every command codex spawned rather than
    codex alone."""

    def __init__(self, cwd, env, on_message):
        self._on_message = on_message
        self._pending = {}
        self._lock = threading.Lock()
        self._last_id = 0
        self._stderr = []
        self._closed = False
        self.proc = subprocess.Popen(
            ["codex", "app-server"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, cwd=cwd, env=env, start_new_session=True)
        threading.Thread(target=self._read, daemon=True).start()
        self._stderr_reader = threading.Thread(target=self._drain_stderr, daemon=True)
        self._stderr_reader.start()

    def handshake(self):
        self.request("initialize", {"clientInfo": _CLIENT,
                                    "capabilities": {"optOutNotificationMethods": _DECLINED}})
        self._send({"method": "initialized", "params": {}})

    def request(self, method, params, timeout=_REQUEST_LIMIT):
        with self._lock:
            self._last_id += 1
            request_id = self._last_id
            answered, box = threading.Event(), {}
            self._pending[request_id] = (answered, box)
        self._send({"id": request_id, "method": method, "params": params})
        if not answered.wait(timeout):
            raise _ServerError("codex app-server did not answer %s within %ds" % (method, timeout))
        if "error" in box:
            error = box["error"] or {}
            raise _ServerError(error.get("message") or ("%s failed" % method))
        return box.get("result") or {}

    @property
    def stderr(self):
        return "".join(chunk for chunk in self._stderr if chunk)

    def close(self):
        if self._closed:
            return
        self._closed = True
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.terminate()
        # On a failed run codex's stderr is the whole diagnosis, and it is read
        # straight after this returns — joining makes it there rather than racing.
        self._stderr_reader.join(timeout=5)

    def terminate(self):
        _terminate_tree(self.proc.pid)

    def _send(self, message):
        try:
            self.proc.stdin.write(json.dumps(message) + "\n")
            self.proc.stdin.flush()
        except (OSError, ValueError) as exc:
            raise _ServerError("codex app-server is not accepting input: %s" % exc)

    def _drain_stderr(self):
        self._stderr.append(self.proc.stderr.read())

    def _read(self):
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                message = json.loads(line)
            except ValueError:
                self._on_message({"method": "error", "params": {
                    "error": {"message": "invalid app-server output: %s" % line}}})
                continue
            if "method" in message and "id" in message:
                # Nothing here answers a server-initiated request, and leaving one
                # unanswered wedges the turn waiting on a reply that never comes.
                try:
                    self._send({"id": message["id"],
                                "error": {"code": -32601,
                                          "message": "codex-run answers no server requests"}})
                except _ServerError:
                    pass
            elif "id" in message:
                with self._lock:
                    slot = self._pending.pop(message["id"], None)
                if slot:
                    slot[1].update(message)
                    slot[0].set()
            try:
                self._on_message(message)
            except OSError as exc:
                # A record write happens here on the reader thread. Turn its failure
                # into the terminal notification the main thread is waiting for.
                self._on_message({"method": "error", "params": {
                    "error": {"message": "cannot write job record: %s" % exc}}})
                self._on_message({"method": "turn/failed", "params": {}})
                return
        # stdout closed: fail everything still waiting rather than let it block to
        # its own timeout, one request at a time.
        with self._lock:
            waiting, self._pending = list(self._pending.values()), {}
        for answered, box in waiting:
            box["error"] = {"message": "codex app-server exited"}
            answered.set()


def _terminate_tree(pid):
    """Signal a process group, then make sure it is gone.

    The group, not the pid: codex runs the model's commands as its own children,
    so signalling one process leaves the shell it was inside running."""
    if not pid:
        return False
    try:
        os.killpg(pid, signal.SIGTERM)
    except OSError:
        return False
    for _ in range(40):
        time.sleep(0.25)
        try:
            os.killpg(pid, 0)
        except OSError:
            return True
    try:
        os.killpg(pid, signal.SIGKILL)
    except OSError:
        pass
    return True


def _alive(pid):
    if not pid:
        return False
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


# --- job records --------------------------------------------------------------------

def _output_paths():
    """The (answer, events, record) triple for one run, sharing a stem.

    Resolves the session through the spine exactly as model_call._record does;
    falls back to /tmp when there is no valid session (the same no-session
    fallback spirit — the runner must still surface its files). The pid +
    high-resolution timestamp stem is unique per parallel invocation, which is
    what lets it serve as the job id."""
    stem = "codex-run-%d-%d" % (os.getpid(), time.time_ns())
    base = _resolve_output_dir()
    return (os.path.join(base, stem + ".txt"),
            os.path.join(base, stem + ".jsonl"),
            os.path.join(base, stem + ".json"))


def _resolve_output_dir():
    sid = session_state.own_session_id()
    if sid and session_state._is_valid_session_id(sid):
        session_dir = session_state._ensure_session(sid)
        if session_dir is not None:
            return session_dir
    return "/tmp"


def _transcript_path():
    """The Claude transcript for this session, recorded by the session-state spine."""
    try:
        with open(os.path.join(_resolve_output_dir(), "transcript"), encoding="utf-8") as fh:
            return fh.read().strip()
    except OSError:
        return ""


def _save_record(record):
    """Write the record whole or not at all — `status` reads these while runs write
    them, and a half-written record reads as a corrupt job rather than a live one."""
    path = record["record"]
    temporary = path + ".writing"
    with open(temporary, "w", encoding="utf-8") as fh:
        json.dump(record, fh, indent=1)
        fh.write("\n")
    os.replace(temporary, path)


def _load_record(path):
    try:
        with open(path, encoding="utf-8") as fh:
            record = json.load(fh)
    except (OSError, ValueError) as exc:
        raise OSError("cannot read job record %s: %s" % (path, exc))
    if not isinstance(record, dict):
        raise OSError("cannot read job record %s: expected a JSON object" % path)
    return record


def _job_dirs(everywhere):
    """Where job records live: this session's directory, and every sibling
    session's when the caller asked for all of them."""
    dirs, seen = [], set()
    for candidate in [_resolve_output_dir()] + (_sibling_dirs() if everywhere else []):
        key = os.path.realpath(candidate)
        if key not in seen and os.path.isdir(candidate):
            seen.add(key)
            dirs.append(candidate)
    return dirs


def _sibling_dirs():
    root = os.path.join(session_state._data_root(), "sessions")
    found = []
    for pattern in (os.path.join(root, "*"), os.path.join(root, "*", "subagents", "*")):
        found.extend(sorted(path for path in glob(pattern) if os.path.isdir(path)))
    found.append("/tmp")
    return found


def _records_in(directory):
    records = []
    for path in sorted(glob(os.path.join(directory, "codex-run-*.json"))):
        try:
            record = _load_record(path)
        except OSError as exc:
            # stderr, not stdout: the lifecycle hooks call this on every Stop and
            # SessionEnd, and their stdout is the harness's channel, not ours.
            print("codex-run: %s" % exc, file=sys.stderr)
            continue
        records.append(_reconcile_record(record))
    return records


def _reconcile_record(record):
    """Make a dead runner's durable record terminal when a reader observes it."""
    if record.get("status") == "running" and not _alive(record.get("pid")):
        record.update(status="failed", phase="failed",
                      error=record.get("error") or "codex-run runner exited unexpectedly",
                      ended_at=int(time.time()))
        _save_record(record)
    return record


def live_jobs(session_id):
    """The live codex jobs owned by one Claude session's record directory."""
    if not session_state._is_valid_session_id(session_id):
        return []
    directory = session_state._session_dir(session_id)
    if not directory:
        return []
    return [record for record in _records_in(directory)
            if record.get("status") == "running" and _alive(record.get("pid"))]


def terminate_job(record, grace=0.25):
    """End a recorded run without ever signalling a Claude process group.

    app-server owns its own process group; the runner does not, so only the
    former is group-signalled.  The short grace fits Claude's SessionEnd budget
    and the runner is then killed directly if its signal handler did not finish.
    """
    server_pid, pid = record.get("server_pid"), record.get("pid")
    if _alive(server_pid):
        try:
            os.killpg(server_pid, signal.SIGTERM)
        except OSError:
            pass
    if _alive(pid):
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
    time.sleep(grace)
    if _alive(server_pid):
        try:
            os.killpg(server_pid, signal.SIGKILL)
        except OSError:
            pass
    if _alive(pid):
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass
    record.update(status="cancelled", phase="session-ended", ended_at=int(time.time()))
    _save_record(record)


def _find_job(token):
    """The one job whose id starts with `token`, or a message saying why not.

    A prefix is an identity claim, not a local convenience. Search every session
    before accepting it so a match in this session cannot silently resume a
    different job than the caller intended."""
    matches = []
    for directory in _job_dirs(True):
        matches.extend(record for record in _records_in(directory)
                       if (record.get("job") or "").startswith(token))
    if len(matches) == 1:
        return matches[0], ""
    if len(matches) > 1:
        return None, ("codex-run: '%s' matches %d jobs: %s"
                      % (token, len(matches),
                         ", ".join(sorted(record["job"] for record in matches)[:5])))
    return None, ("codex-run: no job matching '%s'. Run `codex-run status --all` to list them."
                  % token)


# --- the lifecycle feed -------------------------------------------------------------

# One file per Claude session, appended by every run in it. Each line is written
# in one call and stays under the length limit, so parallel runs appending at once
# interleave whole lines rather than fragments of each other's.
_FEED = "codex-run.feed"
_LINE_LIMIT = 500
_FEED_BURST = 10
_FEED_INTERVAL = 2

def _feed_path():
    return os.path.join(_resolve_output_dir(), _FEED)


def _emit(state, job, agent, detail):
    line = "%s %-8s %s %s %s" % (time.strftime("%H:%M:%S"), state, job, agent, detail)
    try:
        with open(_feed_path() + ".gate", "a+", encoding="utf-8") as gate:
            fcntl.flock(gate, fcntl.LOCK_EX)
            try:
                gate.seek(0)
                tokens, updated = json.load(gate)
            except (ValueError, TypeError):
                tokens, updated = _FEED_BURST, time.time()
            now = time.time()
            tokens = min(_FEED_BURST, tokens + (now - updated) / _FEED_INTERVAL)
            if tokens < 1:
                time.sleep((1 - tokens) * _FEED_INTERVAL)
                now = time.time()
                tokens = min(_FEED_BURST, tokens + (now - updated) / _FEED_INTERVAL)
            gate.seek(0)
            gate.truncate()
            json.dump((tokens - 1, now), gate)
            gate.flush()
            with open(_feed_path(), "a", encoding="utf-8") as fh:
                fh.write(line[:_LINE_LIMIT].rstrip() + "\n")
            fcntl.flock(gate, fcntl.LOCK_UN)
    except OSError as exc:
        raise OSError("cannot append lifecycle feed: %s" % exc)


def _elapsed(seconds):
    seconds = int(seconds)
    return "%ds" % seconds if seconds < 60 else "%dm%02ds" % (seconds // 60, seconds % 60)


# --- one run ------------------------------------------------------------------------

# What the model is doing, named from the item codex opened and retained on the
# job record for the statusline.
_PHASES = {"reasoning": "thinking", "commandExecution": "running",
           "fileChange": "editing", "mcpToolCall": "calling",
           "collabAgentToolCall": "delegating", "webSearch": "searching",
            "agentMessage": "answering"}
_TOOL_ITEMS = {"commandExecution", "fileChange", "mcpToolCall", "collabAgentToolCall"}

_PROMPT_EXCERPT = 160


class _Job:
    """One run's live state and the record it keeps on disk.

    Every exit path — a clean turn, a failed one, a cancelled one, a codex that
    never started — goes through `finish`, so a record left saying `running` means
    the runner itself died rather than that the run is still going."""

    def __init__(self, agent, model, effort, prompt, resumed_from=None):
        answer_path, events_path, record_path = _output_paths()
        self.id = os.path.basename(record_path)[: -len(".json")]
        self.agent = agent
        self.answer = None
        self.error = None
        self.cancelled = False
        self.started = time.time()
        self.last_seen = self.started
        self.last_tool_completed_at = None
        self.done = threading.Event()
        self._record_lock = threading.Lock()
        self.events = None
        self.record = {
            "job": self.id,
            "agent": agent,
            "model": model,
            "effort": effort,
            "status": "running",
            "phase": "starting",
            "activity": "",
            "error": None,
            "fresh_input_tokens": 0,
            "thread": None,
            "turn": None,
            "rollout": None,
            "resumed_from": resumed_from,
            "resumed": None,
            "pid": os.getpid(),
            "server_pid": None,
            "answer": answer_path,
            "events": events_path,
            "record": record_path,
            "session": session_state.own_session_id(),
            "transcript": _transcript_path(),
            "cwd": os.getcwd(),
            "prompt": prompt[:_PROMPT_EXCERPT],
            "started_at": int(self.started),
            "updated_at": int(self.started),
            "ended_at": None,
        }
        _save_record(self.record)
        try:
            self.events = open(events_path, "w", encoding="utf-8")
        except OSError as exc:
            self.error = "cannot create event stream: %s" % exc
            self.save(status="failed", phase="failed", error=self.error,
                      ended_at=int(time.time()))
            return
        try:
            _emit("started", self.id, agent, "%s/%s — %s"
                  % (model, effort, " ".join(prompt.split())[:_PROMPT_EXCERPT]))
        except OSError as exc:
            self.error = str(exc)
            self.save(status="failed", phase="failed", error=self.error,
                      ended_at=int(time.time()))
            self.events.close()
            return

    @property
    def answer_path(self):
        return self.record["answer"]

    def save(self, **fields):
        with self._record_lock:
            self.record.update(fields)
            self.record["updated_at"] = int(time.time())
            _save_record(self.record)

    def request_cancel(self):
        self.cancelled = True

    def handle(self, message):
        """Tee every inbound message to the event stream and read the few that say
        what the run is doing. Items from a subagent thread are recorded but never
        answer for the run: codex spawns its own threads, and taking their final
        message as this run's answer returns the wrong text under the right id."""
        self.last_seen = time.time()
        try:
            self.events.write(json.dumps(message) + "\n")
            self.events.flush()
        except (OSError, ValueError) as exc:
            self.error = "cannot append event stream: %s" % exc
            self.done.set()
            return
        method = message.get("method")
        params = message.get("params") or {}
        if not method or not self._ours(params):
            return
        if method == "item/started":
            item = params.get("item") or {}
            phase = _PHASES.get(item.get("type"))
            if item.get("type") == "commandExecution":
                command = item.get("command") or ""
                try:
                    words = shlex.split(command)
                except ValueError:
                    words = []
                if (len(words) >= 3 and os.path.basename(words[0]) in ("sh", "bash", "zsh")
                        and words[1] == "-lc"):
                    command = words[2]
                self.save(phase=phase, activity=" ".join(command.split()))
            elif phase and phase != self.record["phase"]:
                self.save(phase=phase)
        elif method == "item/completed":
            item = params.get("item") or {}
            if item.get("type") == "commandExecution":
                self.save(activity="")
            if item.get("type") in _TOOL_ITEMS:
                self.last_tool_completed_at = time.time()
            if item.get("type") == "agentMessage" and item.get("text"):
                self.answer = item["text"]
        elif method == "thread/tokenUsage/updated":
            total = ((params.get("tokenUsage") or {}).get("total") or {})
            input_tokens, cached_input_tokens = total.get("inputTokens"), total.get("cachedInputTokens")
            if isinstance(input_tokens, int) and isinstance(cached_input_tokens, int):
                self.save(fresh_input_tokens=max(0, input_tokens - cached_input_tokens))
        elif method == "error":
            error = params.get("error") or params
            self.error = self.error or error.get("message") or "codex reported an error"
        elif method == "turn/completed":
            turn = params.get("turn") or {}
            if turn.get("status") != "completed":
                self.error = self.error or ("turn %s" % turn.get("status"))
            self.done.set()
        elif method == "turn/failed":
            self.error = self.error or "turn failed"
            self.done.set()

    def wait(self, server):
        """Block until the turn ends, the architect cancels it, or codex goes quiet."""
        interrupted = None
        while not self.done.wait(1.0):
            now = time.time()
            if server.proc.poll() is not None:
                self.error = self.error or "codex app-server exited unexpectedly"
                return
            if self.cancelled and interrupted is None:
                interrupted = now
                self.save(phase="cancelling")
                try:
                    server.request("turn/interrupt",
                                   {"threadId": self.record["thread"],
                                    "turnId": self.record["turn"]}, timeout=15)
                except _ServerError:
                    pass
            if interrupted is not None:
                if now - interrupted > _CANCEL_GRACE:
                    return
            elif (self.last_tool_completed_at is not None
                  and now - self.last_tool_completed_at > _POST_TOOL_IDLE_LIMIT):
                self.error = ("codex went silent for %ds after a tool result"
                              % _POST_TOOL_IDLE_LIMIT)
                self._interrupt(server)
                return
            elif now - self.last_seen > _IDLE_LIMIT:
                self.error = self.error or ("codex sent nothing for %ds" % _IDLE_LIMIT)
                self._interrupt(server)
                return

    def _interrupt(self, server):
        try:
            server.request("turn/interrupt",
                           {"threadId": self.record["thread"],
                            "turnId": self.record["turn"]}, timeout=15)
        except _ServerError as exc:
            self.error = "\n".join(part for part in
                                   [self.error, "cannot interrupt turn: %s" % exc] if part)

    def finish(self, status):
        self.save(status=status, phase="done" if status == "ok" else status,
                  error=self.error, ended_at=int(time.time()))
        if self.events is not None:
            self.events.close()
        try:
            _emit(status, self.id, self.agent, "%s %d chars"
                  % (_elapsed(time.time() - self.started), os.path.getsize(self.answer_path)))
        except OSError as exc:
            self.error = str(exc)
            self.save(status="failed", phase="failed", error=self.error,
                      ended_at=int(time.time()))

    def _ours(self, params):
        mine, theirs = self.record.get("thread"), params.get("threadId")
        return mine is None or theirs is None or theirs == mine

def _refuse_harness(agent):
    """The refusal a `harness` declaration earns this run, or None.

    The two refusals are separate because their fixes are: a claude-only agent is
    dispatched elsewhere, while an unrecognized value is a broken definition and
    runs nowhere — sending that one to Claude would only earn a second refusal."""
    declared = _declaration(agent, "harness")
    if declared is None or declared in _HERE:
        return None
    if declared == "claude":
        return ('codex-run: the %s agent declares `harness: claude` — it does not run here.\n'
                '\n'
                'Dispatch it on Claude instead, with the same task prompt:\n'
                '\n'
                '  Agent(subagent_type: "%s", prompt: "<the same task prompt>")'
                % (agent, agent))
    return ('codex-run: the %s agent declares `harness: %s`, which is not a harness.\n'
            '\n'
            'Valid values are `all`, `claude`, and `codex`, and omitting the key means\n'
            '`all`. The comparison is exact, so a wrong case is a wrong value. The agent\n'
            'runs nowhere until this is corrected — an unrecognized declaration denies\n'
            'rather than permits, so a typo cannot quietly widen where an agent runs.\n'
            '\n'
            'Fix the `harness:` line in %s.md, then run it again.'
            % (agent, declared, agent))


def _thread_params(instructions, config, model, cwd):
    return {"cwd": cwd, "model": model, "approvalPolicy": "never",
            "sandbox": "danger-full-access",
            "baseInstructions": instructions, "config": config}


def _dispatch(agent, prompt, resume=None):
    """Run one turn as `agent` and print the whole result on stdout.

    Returns the exit code, non-zero on any failure: a codex that would not start,
    a protocol error, a turn that failed, a cancelled turn, or a run that produced
    no final answer — a turn yielding nothing usable is a failure too.

    `resume` is the record of the job being continued. Its thread is asked for
    first; when codex no longer holds that thread the run starts a fresh one from
    the same settings and says so, because the alternative — a silent fresh thread
    under a resume command — reads exactly like a continuation and is not one."""
    prompt_path = _resolve_agent("@" + agent)
    if prompt_path is None:
        print("codex-run: unknown agent '@%s'. Available: %s"
              % (agent, ", ".join(_available_agents())))
        return 1
    try:
        refusal = _refuse_harness(agent)
        if refusal:
            print(refusal)
            return 1
        model = _codex_model(agent)
        effort = _codex_effort(agent)
        with open(prompt_path, encoding="utf-8") as fh:
            instructions = fh.read()
        config = dict(_CONFIG)
        if _declares_blank_memory(agent):
            config = {**config, **_NO_MEMORY_CONFIG}
    except (OSError, UnicodeDecodeError, ValueError) as exc:
        try:
            job = _Job(agent, _MODEL, _EFFORT, prompt,
                       resumed_from=(resume or {}).get("job"))
        except OSError as record_error:
            print("codex-run: cannot write job record: %s" % record_error)
            return 1
        job.error = "cannot prepare agent: %s" % exc
        body = "[no answer — run failed]\n\n%s" % job.error
        try:
            with open(job.answer_path, "w", encoding="utf-8") as fh:
                fh.write(body + "\n")
        except OSError as answer_error:
            job.error = "cannot write preparation failure answer: %s" % answer_error
            body += "\n\n%s" % job.error
        job.finish("failed")
        print(body)
        _print_trailer(job.record)
        return 1
    env = dict(os.environ)
    definition = _definition_path(agent)
    if definition:
        # The path rather than the name on purpose: the roster resolution that
        # produced it is subtle, and a hook re-deriving it would be a second
        # implementation of the thing that must not drift.
        env[agent_memory.AGENT_FILE_VAR] = definition
    else:
        # An inherited value from an outer codex-run would gate this run as the
        # wrong agent, which is worse than not gating it.
        env.pop(agent_memory.AGENT_FILE_VAR, None)

    try:
        job = _Job(agent, model, effort, prompt,
                   resumed_from=(resume or {}).get("job"))
    except OSError as exc:
        print("codex-run: cannot write job record: %s" % exc)
        return 1
    if job.error:
        body = "[no answer — run failed]\n\n%s" % job.error
        try:
            with open(job.answer_path, "w", encoding="utf-8") as fh:
                fh.write(body + "\n")
        except OSError as exc:
            job.error = "cannot write final answer: %s" % exc
            body += "\n\n%s" % job.error
        job.finish("failed")
        print(body)
        _print_trailer(job.record)
        return 1
    try:
        signal.signal(signal.SIGTERM, lambda *_: job.request_cancel())
        signal.signal(signal.SIGINT, lambda *_: job.request_cancel())
    except ValueError:
        pass  # not the main thread; cancellation is reachable only from it

    server, lost = None, ""
    try:
        server = _Server(job.record["cwd"], env, job.handle)
        job.save(server_pid=server.proc.pid)
        server.handshake()
        params = _thread_params(instructions, config, model, job.record["cwd"])
        if resume:
            try:
                thread = server.request("thread/resume",
                                        dict(params, threadId=resume.get("thread")))["thread"]
                job.record["resumed"] = True
            except _ServerError as exc:
                lost = str(exc)
                thread = server.request("thread/start", params)["thread"]
                job.record["resumed"] = False
        else:
            thread = server.request("thread/start", params)["thread"]
        job.save(thread=thread.get("id"), rollout=thread.get("path"))
        turn = server.request("turn/start", {
            "threadId": thread.get("id"),
            "input": [{"type": "text", "text": prompt}],
            "model": model,
            "effort": effort})["turn"]
        job.save(turn=turn.get("id"), phase="thinking")
        job.wait(server)
    except _ServerError as exc:
        job.error = job.error or str(exc)
    except OSError as exc:
        # A codex that is not installed or not on PATH — surfaced in the answer
        # slot like any other failure, never as a traceback.
        job.error = job.error or ("cannot run codex app-server: %s" % exc)
    finally:
        if server is not None:
            server.close()

    if job.answer is None and not job.error and not job.cancelled:
        job.error = "turn completed but produced no message"
    status = "cancelled" if job.cancelled else (
        "failed" if (job.error or job.answer is None) else "ok")

    # On a failed run with no answer, codex's stderr is the only diagnosis there
    # is — persist it as the output so the failing run is debuggable on disk.
    trouble = "\n".join(part for part in
                        [job.error, (server.stderr.strip() if server else "")] if part)
    body = job.answer if job.answer is not None else (
        "[no answer — run %s]\n\n%s" % (status, trouble) if trouble
        else "[no answer — run %s]" % status)
    try:
        with open(job.answer_path, "w", encoding="utf-8") as fh:
            fh.write(body + "\n")
    except OSError as exc:
        job.error = "cannot write final answer: %s" % exc
        status = "failed"
        body += "\n\n%s" % job.error

    job.finish(status)
    status = job.record["status"]

    print(body)
    if status != "ok" and trouble and job.answer is not None:
        print("\ncodex error output:\n%s" % trouble)
    if lost:
        print("\ncodex-run: DID NOT RESUME — %s. Started fresh thread %s from job %s's settings;\n"
              "this turn ran with none of that thread's history."
              % (lost, job.record["thread"], resume.get("job")))
    _print_trailer(job.record)
    return 0 if status == "ok" else 1


def _print_trailer(record):
    print(_TRAILER)
    print("status:  %s" % record.get("status"))
    print("agent:   %s" % record.get("agent"))
    print("model:   %s  (effort %s)" % (record.get("model"), record.get("effort")))
    print("job:     %s" % record.get("job"))
    print("thread:  %s" % (record.get("thread") or ""))
    print("output:  %s" % record.get("answer"))
    print("events:  %s" % record.get("events"))


# --- the job surface ----------------------------------------------------------------

def _display_status(record):
    return record.get("status") or "?"


def _cmd_status(argv):
    everywhere = "--all" in argv
    records = []
    for directory in _job_dirs(everywhere):
        records.extend(_records_in(directory))
    if not records:
        print("codex-run: no jobs%s" % ("" if everywhere else " in this session (try --all)"))
        return 0
    # The job id breaks the tie: its stem carries a nanosecond stamp, where the
    # record's timestamp is whole seconds and two parallel runs share one.
    records.sort(key=lambda record: (record.get("started_at") or 0, record.get("job") or ""),
                 reverse=True)
    print("%-38s %-18s %-9s %-10s %-7s %s"
          % ("JOB", "AGENT", "STATUS", "PHASE", "TIME", "PROMPT"))
    for record in records:
        ended = record.get("ended_at") or int(time.time())
        print("%-38s %-18s %-9s %-10s %-7s %s"
              % (record.get("job"), record.get("agent"), _display_status(record),
                 record.get("phase"), _elapsed(ended - (record.get("started_at") or ended)),
                 " ".join((record.get("prompt") or "").split())[:60]))
    return 0


def _cmd_result(argv):
    record, complaint = _find_job(argv[0])
    if record is None:
        print(complaint)
        return 1
    try:
        with open(record.get("answer") or "", encoding="utf-8") as fh:
            print(fh.read().rstrip())
    except OSError:
        print("[no answer on disk — the job is %s]" % _display_status(record))
    _print_trailer(dict(record, status=_display_status(record)))
    return 0


def _cmd_history(argv):
    record, complaint = _find_job(argv[0])
    if record is None:
        print(complaint)
        return 1
    print("job:        %s" % record.get("job"))
    print("agent:      %s" % record.get("agent"))
    print("thread:     %s" % (record.get("thread") or ""))
    print("rollout:    %s" % (record.get("rollout") or "[codex wrote none]"))
    print("transcript: %s" % (record.get("transcript") or "[no Claude transcript recorded]"))
    return 0


def _cmd_events(argv):
    record, complaint = _find_job(argv[0])
    if record is None:
        print(complaint)
        return 1
    tail = 0
    if "--tail" in argv:
        position = argv.index("--tail")
        if position + 1 >= len(argv) or not argv[position + 1].isdigit():
            print("codex-run: --tail needs a number")
            return 2
        tail = int(argv[position + 1])
    try:
        with open(record.get("events") or "", encoding="utf-8") as fh:
            lines = fh.read().splitlines()
    except OSError:
        print("codex-run: no event stream on disk for %s" % record.get("job"))
        return 1
    for line in (lines[-tail:] if tail else lines):
        print(line)
    return 0


def _summarize(item):
    """One line of activity for an item codex finished, or None when it is noise."""
    kind = item.get("type")
    if kind == "commandExecution":
        return "command  %s (exit %s)" % (" ".join((item.get("command") or "").split())[:200],
                                          item.get("exitCode"))
    if kind == "fileChange":
        return "edit     %s" % ", ".join(
            change.get("path", "?") for change in (item.get("changes") or []))[:200]
    if kind == "mcpToolCall":
        return "tool     %s/%s (%s)" % (item.get("server"), item.get("tool"), item.get("status"))
    if kind == "webSearch":
        return "search   %s" % (item.get("query") or "")[:200]
    if kind == "agentMessage":
        return "message  %s" % " ".join((item.get("text") or "").split())[:200]
    if kind == "collabAgentToolCall":
        return "subagent %s (%s)" % (item.get("tool"), item.get("status"))
    return None


def _cmd_log(argv):
    record, complaint = _find_job(argv[0])
    if record is None:
        print(complaint)
        return 1
    try:
        with open(record.get("events") or "", encoding="utf-8") as fh:
            lines = fh.read().splitlines()
    except OSError:
        print("codex-run: no event stream on disk for %s" % record.get("job"))
        return 1
    printed = 0
    for line in lines:
        try:
            message = json.loads(line)
        except ValueError:
            continue
        method, params = message.get("method"), message.get("params") or {}
        if method == "turn/started":
            print("turn started")
        elif method == "turn/completed":
            print("turn %s" % (params.get("turn") or {}).get("status"))
        elif method == "error":
            print("error    %s" % ((params.get("error") or params).get("message") or ""))
        elif method == "item/completed":
            summary = _summarize(params.get("item") or {})
            if summary:
                print(summary)
                printed += 1
    if not printed:
        print("[no activity recorded — the job is %s]" % _display_status(record))
    return 0


def _cmd_cancel(argv):
    record, complaint = _find_job(argv[0])
    if record is None:
        print(complaint)
        return 1
    if _display_status(record) not in ("running",):
        print("codex-run: %s is already %s" % (record.get("job"), _display_status(record)))
        return 0
    # The runner owns the connection, so it is the only process that can send
    # turn/interrupt on it. SIGTERM asks it to; the process tree is the fallback
    # for a runner too wedged to answer its own signal.
    try:
        os.kill(record["pid"], signal.SIGTERM)
    except OSError as exc:
        print("codex-run: cannot signal the runner for %s: %s" % (record.get("job"), exc))
        return 1
    print("codex-run: interrupting %s (%s)…" % (record.get("job"), record.get("agent")))
    for _ in range(int((_CANCEL_GRACE + 20) / 0.5)):
        time.sleep(0.5)
        try:
            current = _load_record(record["record"])
        except OSError as exc:
            print("codex-run: %s" % exc)
            return 1
        if (current.get("status") or "") != "running":
            print("codex-run: %s is %s" % (record.get("job"), current.get("status")))
            return 0
    _terminate_tree(record.get("server_pid"))
    try:
        os.kill(record["pid"], signal.SIGKILL)
    except OSError as exc:
        record.update(status="failed", phase="failed",
                      error="cannot force-cancel runner: %s" % exc,
                      ended_at=int(time.time()))
        _save_record(record)
        print("codex-run: cannot force-cancel %s: %s" % (record.get("job"), exc))
        return 1
    record.update(status="cancelled", phase="cancelled", ended_at=int(time.time()))
    _save_record(record)
    print("codex-run: %s did not stop on its own — killed the codex process tree."
          % record.get("job"))
    return 1


def _cmd_watch(argv):
    """Follow this session's lifecycle feed, one line per meaningful event.

    Runs until it is killed. The recent backlog is printed first so a monitor
    attaching mid-run sees the jobs already in flight instead of an empty screen
    until the next line lands."""
    path = _feed_path()
    print("watching %s" % path)
    position = 0
    try:
        with open(path, encoding="utf-8") as fh:
            recent = fh.read().splitlines()
        for line in recent[-5:]:
            print(line)
        position = sum(len(line) + 1 for line in recent)
    except OSError as exc:
        print("codex-run: cannot read lifecycle feed %s: %s" % (path, exc))
        return 1
    while True:
        try:
            with open(path, encoding="utf-8") as fh:
                fh.seek(position)
                fresh = fh.read()
                position = fh.tell()
            for line in fresh.splitlines():
                print(line, flush=True)
        except OSError as exc:
            print("codex-run: cannot read lifecycle feed %s: %s" % (path, exc))
            return 1
        time.sleep(1.0)


# --- entrypoint ---------------------------------------------------------------------

_USAGE = ('Usage:\n'
          '  codex-run @<agent> "<prompt>"      run codex as <agent>\n'
          '  codex-run resume <job> "<msg>"     continue a prior run\n'
          '  codex-run status [--all]           list jobs (--all scans every session)\n'
          '  codex-run result <job>             print a run\'s final answer\n'
          '  codex-run log <job>                print a run\'s activity\n'
          '  codex-run events <job> [--tail N]  print a run\'s raw event stream\n'
          '  codex-run history <job>            print the codex rollout and Claude transcript\n'
          '  codex-run cancel <job>             interrupt a running turn\n'
          '  codex-run watch                    follow this session\'s lifecycle feed\n'
          '  A job id may be shortened to any prefix that names one job.\n'
          '  Pass - as the prompt/message to read it from stdin — immune to shell quoting.')


def _read_prompt(arg):
    # "-" reads the prompt from stdin so no shell-quoting of the argv form can
    # mangle or kill the run — the quoting-death class becomes unrepresentable.
    return sys.stdin.read() if arg == "-" else arg


_ONE_JOB = {"result": _cmd_result, "log": _cmd_log, "events": _cmd_events,
            "history": _cmd_history, "cancel": _cmd_cancel}


def main(argv):
    # Every message — errors included — goes to stdout so the result reads
    # cleanly with no downstream parsing.
    command = argv[0] if argv else ""

    if command == "resume":
        if len(argv) != 3:
            print("codex-run: resume needs a job id and a message\n" + _USAGE)
            return 2
        record, complaint = _find_job(argv[1])
        if record is None:
            print(complaint)
            return 1
        agent = record.get("agent") or ""
        if agent not in _available_agents():
            # Loud stop, not a silent continuation: without the founding agent the
            # run would answer as codex's global default with the thread's history
            # intact, which reads as the same agent and is not.
            print("codex-run: cannot resume %s — its agent '%s' is not a runnable agent. "
                  "Available: %s" % (record.get("job"), agent, ", ".join(_available_agents())))
            return 1
        if not record.get("thread"):
            print("codex-run: cannot resume %s — it never reached a codex thread."
                  % record.get("job"))
            return 1
        return _dispatch(agent, _read_prompt(argv[2]), resume=record)

    if command == "status":
        return _cmd_status(argv[1:])

    if command == "watch":
        return _cmd_watch(argv[1:])

    if command in _ONE_JOB:
        if len(argv) < 2:
            print("codex-run: %s needs a job id\n%s" % (command, _USAGE))
            return 2
        return _ONE_JOB[command](argv[1:])

    if len(argv) != 2 or not command.startswith("@"):
        print(_USAGE)
        return 2

    if _resolve_agent(command) is None:
        print("codex-run: unknown agent '%s'. Available: %s"
              % (command, ", ".join(_available_agents())))
        return 1
    return _dispatch(command[1:], _read_prompt(argv[1]))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
