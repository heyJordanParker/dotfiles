"""codex_run — run codex as one of our named agents, mechanically.

The orchestrating agent used to hand-assemble the whole `codex exec` invocation
every time it dispatched codex as one of our agents: the flags, a `/tmp` output
path, an inline JSONL parser, a failure grep. That assembly was duplicated across
the codex-agent and codex-review skills and needed no judgment. This module owns
all of it, so the agent only writes the task prompt.

`codex-run @<agent> "<prompt>"` resolves `@<agent>` to that agent's
frontmatter-stripped instruction body (`<roster>/<name>.prompt.md`, the same
artifact that boots codex as the CTO) and runs codex with it as the
base-instructions override. `codex-run resume <session_id> "<msg>"` continues a
prior run. An unknown agent exits non-zero listing the available agents.

There is more than one roster. The active config root's `agents/` is searched
first and the shared `~/.agents/agents` second, because a profile is its own
config root with its own agents and those are real agents while that profile is
active — resolving only the shared roster made every profile-only agent
unreachable on codex. Active root first means a name held by both runs as the
profile's, matching which definition governs on the Claude side; keeping the
shared roster behind it means entering a profile adds a roster rather than
losing one. The instructions and the memory declaration always come from the
same roster, so a profile's copy of an agent cannot run under another roster's
declaration.

An agent whose `<name>.md` frontmatter declares `memory: none` runs with both of
codex's memory providers switched off — the honcho MCP server and codex's own
[memories] — so no Memory reaches the run and none is written back.

An agent whose frontmatter declares `codex-model` runs on that model instead of
the default below, and one declaring `effort` runs at that effort. Both are read
off the definition file per run rather than baked into the generated artifacts,
so they reach a resumed run — where the agent is recovered, not passed — by the
same route the memory declaration does, and an edit takes effect without
regenerating anything. `effort` is the same field Claude reads natively, so an
agent states its effort once and both harnesses honour it.

`resume` takes a session and a message and nothing else, so the founding agent is
recovered rather than passed. The source is codex's own record: the thread's
rollout opens with a `session_meta` carrying the `base_instructions` the run was
founded on, and those instructions name their agent — every generated .prompt.md
opens with an inert HTML-comment marker holding the agent's name, so the name
rides into the record and comes back out as an exact lookup. Identity therefore
survives any rewording of an agent's text; before the marker it was inferred by
matching the recorded prose back against the corpus, and a reworded opening line
orphaned every thread founded before that edit. A marker naming something outside
the roster resolves to nothing, so the runnable set stays the allowlist.

Threads founded before the marker carry no name and still resolve through that
prose matching. The continuation then runs with the recovered agent's
instructions override and that agent's memory declaration — the same two things
the founding run had. Because the record is codex's, recovery works on every
thread it ever wrote, including those founded before this code existed, and it
states what actually reached the model rather than what this wrapper meant to
send.

An unidentifiable thread fails the run rather than continuing: answering as
codex's global default agent, silently and under a different character while the
surviving history keeps the replies on topic, is the defect this recovery removes.

Our shared Python guards govern the run, not codex's sandbox — the architect does
not want codex sandboxes — so every run passes
`--dangerously-bypass-approvals-and-sandbox`. Our hooks are our own vetted
sources, so every run also passes `--dangerously-bypass-hook-trust`, which runs
them without codex's per-command trust gate (and without reading or writing its
trust table).

Output (the final answer and the raw event stream) lands in the session's own
directory via the session-state helper, never `/tmp`, with collision-free names
for parallel runs, and the same no-session fallback the model runner uses.

The stream is parsed for the final answer and the session id; the run exits
non-zero on a process failure, a `turn.failed` event (a turn that fails but exits
zero still trips this), or a run that produced no final answer (a turn yielding
nothing usable is a failure too). codex's stderr is captured, and on a failed run
with no answer it is surfaced as the result on stdout and on disk — so a failed
run is diagnosable rather than a bare "[no answer]".

The result is printed to stdout, ready to read with no downstream parsing: the
final answer, a `--- codex-run ---` delimiter, then the status, the session id
(for resume), and the on-disk output and events paths. Nothing the caller needs
goes to stderr or requires a pipe — the wrapper is the whole interface.

Stdlib only, matching the other hooks. Run via the `codex-run` launcher, which
exec's `main()`.
"""

import os
import subprocess
import sys
import time

import agent_memory
import session_state

AGENTS_DIR = os.path.expanduser("~/.agents/agents")

# The base-instructions override codex applies per run — the same key the global
# config.toml uses to boot the CTO, set here per-agent instead.
_INSTRUCTIONS_KEY = "model_instructions_file"

# No sandbox and no hook-trust gate: our shared Python guards govern the run, and
# the hooks are our own vetted sources.
#
# Model and effort default here rather than being inherited from config.toml:
# that file configures the interactive session, and an agent run is a different
# job — a scoped task dispatched by a coordinator, not an open-ended session.
# Defaulting means the agents cannot silently drift when the interactive default
# changes. An agent overrides either for itself in its frontmatter: `codex-model`
# for the model, the codex-side counterpart of `model`, which names a Claude model
# and so cannot serve here; and `effort`, which is one field for both harnesses,
# because the four words Claude's key takes are the four codex takes.
_MODEL = "gpt-5.6-sol"
_EFFORT = "medium"

# What an agent may declare: the five levels `claude --effort` lists, each of
# which codex also accepts verbatim as model_reasoning_effort, so a declaration
# crosses to either harness untranslated. `xhigh` and `max` are distinct levels on
# both — translating one onto the other quietly ran an agent at a tier it did not
# ask for, and omitting `xhigh` rejected the tier most likely to be declared.
_EFFORTS = ("low", "medium", "high", "xhigh", "max")

_BASE_FLAGS = ["--json", "--skip-git-repo-check",
               "--dangerously-bypass-approvals-and-sandbox",
               "--dangerously-bypass-hook-trust"]

# Delimits the answer from the metadata trailer on stdout, so the result reads
# cleanly with no downstream parsing: everything above the line is codex's answer,
# everything below is status / session / output / events.
_TRAILER = "--- codex-run ---"


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
    """The reasoning effort `name` runs at: its `effort` declaration, or _EFFORT.

    `effort` is one field across both harnesses — Claude reads the declaration
    natively and codex takes the same word verbatim, so declaring `high` cannot
    mean high on one harness and something else on the other. Unlike the model,
    the value is checked: the vocabulary is closed and shared, so a word outside
    it is a typo in the definition rather than a level either harness has."""
    declared = _declaration(name, "effort")
    if not declared:
        # A key with no value is an undeclared key, the same reading `codex-model`
        # gives it — two adjacent declarations of the same shape must not resolve
        # a blank in opposite directions.
        return _EFFORT
    if declared not in _EFFORTS:
        raise ValueError("agent %r declares unknown effort %r; valid: %s"
                         % (name, declared, ", ".join(_EFFORTS)))
    return declared


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


# --- which agent founded a thread, read from codex's own record --------------------

CODEX_HOME = os.path.expanduser("~/.codex")


def _rollout_path(session):
    """The rollout codex wrote for `session`, or None.

    The id is validated before it reaches the glob, so a session argument cannot
    smuggle pattern metacharacters into the search."""
    from glob import glob
    if not session_state._is_valid_session_id(session):
        return None
    name = "rollout-*-%s.jsonl" % session
    for pattern in (os.path.join(CODEX_HOME, "sessions", "**", name),
                    os.path.join(CODEX_HOME, "archived_sessions", name)):
        for match in sorted(glob(pattern, recursive=True)):
            return match
    return None


def _founding_instructions(session):
    """The base instructions the thread was founded on, from its rollout's
    session_meta — codex's own record of the run, so it states what actually
    reached the model rather than what this wrapper meant to send."""
    import json
    path = _rollout_path(session)
    if path is None:
        return None
    try:
        with open(path, encoding="utf-8") as fh:
            first = fh.readline()
    except OSError:
        return None
    try:
        event = json.loads(first)
    except ValueError:
        return None
    payload = event.get("payload")
    if event.get("type") != "session_meta" or not isinstance(payload, dict):
        return None
    instructions = payload.get("base_instructions")
    if isinstance(instructions, dict):
        return instructions.get("text")
    return instructions if isinstance(instructions, str) else None


# The name-carrying line scripts/agents.py writes at the top of every generated
# .prompt.md. An HTML comment renders as nothing and instructs nothing, so it is
# inert to the model reading the instructions and carries identity only.
_MARKER_OPEN = "<!-- codex-run agent: "
_MARKER_CLOSE = "-->"


def _marked_agent(text):
    """The agent name the marker line at the top of `text` declares, or None.

    Only the first line is considered, so a marker quoted further down the body
    cannot claim an identity."""
    head = text.strip().splitlines()[:1]
    if not head:
        return None
    line = head[0].strip()
    if not line.startswith(_MARKER_OPEN) or not line.endswith(_MARKER_CLOSE):
        return None
    return line[len(_MARKER_OPEN):-len(_MARKER_CLOSE)].strip() or None


def _without_marker(text):
    text = text.strip()
    if _marked_agent(text) is None:
        return text
    return text.split("\n", 1)[1].strip() if "\n" in text else ""


def _founding_agent(session):
    """The agent whose instructions founded `session`, or None if none matches.

    The recorded instructions name their own agent when the thread was founded
    after the marker existed, so identity is an exact lookup rather than a guess
    read off prose — rewording an agent's text cannot orphan such a thread. The
    name still resolves through `_available_agents`, so a stale or tampered
    marker naming something outside the roster resolves to nothing.

    Threads founded before the marker carry no name, so the text matching stays
    for them: whole text first, then the first line, both compared with the
    marker stripped off the corpus so the marker's arrival did not move the
    opener out from under every pre-existing thread. A first line claimed by more
    than one agent resolves to nothing rather than to a guess."""
    recorded = _founding_instructions(session)
    if not recorded or not recorded.strip():
        return None
    named = _marked_agent(recorded)
    if named:
        return named if named in _available_agents() else None
    body = _without_marker(recorded)
    head = body.splitlines()[0].strip() if body else ""
    by_head = []
    for name in _available_agents():
        directory = _agent_dir(name)
        if directory is None:
            continue
        try:
            with open(os.path.join(directory, name + ".prompt.md"), encoding="utf-8") as fh:
                source = _without_marker(fh.read())
        except OSError:
            continue
        if source == body:
            return name
        if source.splitlines()[:1] == [head]:
            by_head.append(name)
    return by_head[0] if len(by_head) == 1 else None


# --- output storage: session dir via the spine, /tmp fallback ---------------------

def _output_paths():
    """A collision-free (answer_file, events_file) pair under the session dir.

    Resolves the session through the spine exactly as model_call._record does;
    falls back to /tmp when there is no valid session (the same no-session
    fallback spirit — the runner must still surface its files). The pid +
    high-resolution timestamp stem is unique per parallel invocation."""
    stem = "codex-run-%d-%d" % (os.getpid(), time.time_ns())
    base = _resolve_output_dir()
    return os.path.join(base, stem + ".txt"), os.path.join(base, stem + ".jsonl")


def _resolve_output_dir():
    sid = session_state.own_session_id()
    if sid and session_state._is_valid_session_id(sid):
        session_dir = session_state._ensure_session(sid)
        if session_dir is not None:
            return session_dir
    return "/tmp"


# --- the codex invocation ---------------------------------------------------------

def _run(cmd, events_path):
    """Run codex, tee the event stream to events_path, return (returncode, answer,
    session_id, turn_failed, stderr). The stream is the single source: the final
    answer is the last agent_message item.completed, the session id is
    thread.started's thread_id, a turn.failed event marks a failed turn even on a
    zero exit. codex's stderr is captured so a failed run is diagnosable — on
    failure it carries the actual error text the event stream never produced.

    Both pipes are drained concurrently: stderr on a reader thread while the main
    loop drains stdout. Reading stdout to exhaustion before touching stderr would
    deadlock on a run that writes enough stderr to fill its pipe buffer — codex
    blocks writing stderr, stops producing stdout, and the runner blocks reading
    stdout that never comes. Draining both at once means neither pipe can fill and
    stall the other, regardless of how much codex writes to either."""
    import json
    import threading

    answer = None
    session = ""
    turn_failed = False
    try:
        proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True)
    except Exception as exc:
        # Surface the reason in the answer slot so _dispatch prints it on stdout.
        return 1, "codex-run: failed to launch codex: %s" % exc, "", False, ""

    stderr_chunks = []
    stderr_reader = threading.Thread(target=lambda: stderr_chunks.append(proc.stderr.read()))
    stderr_reader.start()

    with open(events_path, "w", encoding="utf-8") as events:
        for line in proc.stdout:
            events.write(line)
            line = line.strip()
            if not line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            etype = event.get("type")
            if etype == "thread.started":
                session = event.get("thread_id") or session
            elif etype == "turn.failed":
                turn_failed = True
            elif etype == "item.completed":
                item = event.get("item")
                if isinstance(item, dict) and item.get("type") == "agent_message" and item.get("text"):
                    answer = item["text"]
    stderr_reader.join()
    returncode = proc.wait()
    return returncode, answer, session, turn_failed, stderr_chunks[0]


# An agent declaring `memory: none` runs with every memory provider off, not just
# one. codex has two: the honcho MCP server, whose tools are never registered so
# Memory is out of reach for reads and writes alike rather than gated after
# the fact; and codex's own [memories], which needs no tool call at all because
# it injects a Memory section and MEMORY_SUMMARY straight into the run. Switching
# off only honcho left a blank-declared agent reading Memory through
# the second provider — `generate_memories` covers the write direction too, so a
# one-shot run cannot deposit anything for a later run to read.
_NO_MEMORY_FLAGS = ["-c", "mcp_servers.honcho.enabled=false",
                    "-c", "memories.use_memories=false",
                    "-c", "memories.generate_memories=false"]


def _dispatch(prompt_path, prompt, agent, resume_id=None, blank_memory=False):
    """Build the codex command, run it, store the answer, print the clean result.

    Returns the process exit code, raised to 1 on a turn.failed event or a run
    that produced no final answer — a turn that yields nothing usable is a failure,
    not a success.

    The whole result lands on stdout, ready to read with no downstream parsing:
    the final answer (or, on a failed run with no answer, codex's captured error
    text), then a delimited trailer carrying the status, the session id (for
    resume), and the on-disk output and events paths. Nothing the caller needs is
    anywhere else — no stderr, no pipe."""
    answer_path, events_path = _output_paths()
    memory = _NO_MEMORY_FLAGS if blank_memory else []
    # The instructions override rides on a resume too. codex does not persist it
    # with the thread: a continuation without it runs on the global default
    # instructions, which is how a thread founded as one agent came back as
    # another with nothing in the output saying so.
    instructions = ["-c", "%s=%s" % (_INSTRUCTIONS_KEY, prompt_path)]
    # The model rides a resume for the same reason the instructions do: it is the
    # agent's, resolved from the recovered agent, not something codex kept with
    # the thread.
    model = _codex_model(agent)
    try:
        effort = _codex_effort(agent)
    except ValueError as exc:
        # Everything the caller needs is on stdout, errors included — a traceback
        # out of a wrapper whose whole interface is its printed result is not a
        # usable failure.
        print("codex-run: %s" % exc)
        return 1
    head = ["codex", "exec"] + (["resume", resume_id] if resume_id is not None else [])
    cmd = (head + _BASE_FLAGS + ["-m", model]
           + ["-c", "model_reasoning_effort=%s" % effort]
           + memory + instructions + [prompt])

    returncode, answer, session, turn_failed, stderr = _run(cmd, events_path)

    # No final answer is a failure — a run that exits zero but produced nothing
    # usable is not a success, the same as a process failure or a failed turn.
    # A resume that comes back under a different thread id did not resume — codex
    # started a fresh thread (the silent death mode of resuming an exhausted
    # session), so the requested session's context is gone and the run failed.
    resumed_fresh = resume_id is not None and session and session != resume_id
    failed = returncode != 0 or turn_failed or answer is None or resumed_fresh
    status = "failed" if failed else "ok"

    # On a failed run with no answer, codex's stderr is the only diagnosis there
    # is — persist it as the output so the failing run is debuggable on disk.
    error = stderr.strip()
    disk = answer if answer is not None else (
        "[no answer — run %s]\n\n%s" % (status, error) if error
        else "[no answer — run %s]" % status)
    with open(answer_path, "w", encoding="utf-8") as fh:
        fh.write(disk + "\n")

    print(disk)
    if failed and error:
        print("\ncodex error output:\n%s" % error)
    print(_TRAILER)
    print("status:  %s" % status)
    # Named on every run, resumes included: the identity a continuation runs under
    # is now resolved rather than assumed, so it is worth showing.
    print("agent:   %s" % agent)
    # Beside the agent for the same reason: which model a run used is now the
    # agent's declaration rather than one constant, so a run states it.
    print("model:   %s  (effort %s)" % (model, effort))
    if resumed_fresh:
        print("resume:  DID NOT RESUME — codex started fresh thread %s instead of %s"
              % (session, resume_id))
    print("session: %s" % session)
    print("output:  %s" % answer_path)
    print("events:  %s" % events_path)

    return 1 if failed else 0


# --- entrypoint -------------------------------------------------------------------

_USAGE = ('Usage:\n'
          '  codex-run @<agent> "<prompt>"        run codex as <agent>\n'
          '  codex-run resume <session> "<msg>"   continue a prior run\n'
          '  Pass - as the prompt/message to read it from stdin — immune to shell quoting.')


def _read_prompt(arg):
    # "-" reads the prompt from stdin so no shell-quoting of the argv form can
    # mangle or kill the run — the quoting-death class becomes unrepresentable.
    return sys.stdin.read() if arg == "-" else arg


def main(argv):
    # Every message — errors included — goes to stdout so the result reads
    # cleanly with no downstream parsing.
    if len(argv) >= 1 and argv[0] == "resume":
        if len(argv) != 3:
            print("codex-run: resume needs a session id and a message\n" + _USAGE)
            return 2
        session = argv[1]
        agent = _founding_agent(session)
        prompt_path = _resolve_agent("@" + agent) if agent else None
        if prompt_path is None:
            # Loud stop, not a silent continuation: without the founding agent the
            # run would answer as codex's global default with the thread's history
            # intact, which reads as the same agent and is not.
            print("codex-run: cannot resume %s — its founding agent is unidentifiable%s"
                  % (session, "" if agent is None else " ('%s' is not a runnable agent)" % agent))
            return 1
        return _dispatch(prompt_path, _read_prompt(argv[2]), agent, resume_id=session,
                         blank_memory=_declares_blank_memory(agent))

    if len(argv) != 2 or not argv[0].startswith("@"):
        print(_USAGE)
        return 2

    prompt_path = _resolve_agent(argv[0])
    if prompt_path is None:
        print("codex-run: unknown agent '%s'. Available: %s"
              % (argv[0], ", ".join(_available_agents())))
        return 1
    return _dispatch(prompt_path, _read_prompt(argv[1]), argv[0][1:],
                     blank_memory=_declares_blank_memory(argv[0][1:]))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
