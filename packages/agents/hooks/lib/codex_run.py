"""codex_run — run codex as one of our named agents, mechanically.

The orchestrating agent used to hand-assemble the whole `codex exec` invocation
every time it dispatched codex as one of our agents: the flags, a `/tmp` output
path, an inline JSONL parser, a failure grep. That assembly was duplicated across
the codex-agent and codex-review skills and needed no judgment. This module owns
all of it, so the agent only writes the task prompt.

`codex-run @<agent> "<prompt>"` resolves `@<agent>` to that agent's
frontmatter-stripped instruction body (`~/.agents/agents/<name>.prompt.md`, the
same artifact that boots codex as the CTO) and runs codex with it as the
base-instructions override. `codex-run resume <session_id> "<msg>"` continues a
prior run. An unknown agent exits non-zero listing the available agents.

Our shared Python guards govern the run, not codex's sandbox — the architect does
not want codex sandboxes — so every run passes
`--dangerously-bypass-approvals-and-sandbox`.

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

import session_state

AGENTS_DIR = os.path.expanduser("~/.agents/agents")

# The base-instructions override codex applies per run — the same key the global
# config.toml uses to boot the CTO, set here per-agent instead.
_INSTRUCTIONS_KEY = "model_instructions_file"

# No sandbox: our shared Python guards govern the run instead of codex's sandbox.
_BASE_FLAGS = ["--json", "--skip-git-repo-check",
               "--dangerously-bypass-approvals-and-sandbox"]

# Delimits the answer from the metadata trailer on stdout, so the result reads
# cleanly with no downstream parsing: everything above the line is codex's answer,
# everything below is status / session / output / events.
_TRAILER = "--- codex-run ---"


def _available_agents():
    """The agent names runnable as @<agent> — one per <name>.prompt.md."""
    if not os.path.isdir(AGENTS_DIR):
        return []
    names = []
    for entry in sorted(os.listdir(AGENTS_DIR)):
        if entry.endswith(".prompt.md"):
            names.append(entry[: -len(".prompt.md")])
    return names


def _resolve_agent(token):
    """`@<agent>` → the absolute path to its prompt.md, or None if unknown.

    The runnable set is exactly the named-agent allowlist (`_available_agents`),
    so the name must be one of those exact names — a name carrying path segments
    (`@../../something`) is rejected as unknown rather than joined onto the agents
    dir, which would let it reach an instruction file outside the named set."""
    if not token.startswith("@"):
        return None
    name = token[1:]
    if name not in _available_agents():
        return None
    return os.path.join(AGENTS_DIR, name + ".prompt.md")


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


def _dispatch(prompt_path, prompt, resume_id=None):
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
    if resume_id is not None:
        cmd = ["codex", "exec", "resume", resume_id] + _BASE_FLAGS + [prompt]
    else:
        cmd = ["codex", "exec"] + _BASE_FLAGS + \
              ["-c", "%s=%s" % (_INSTRUCTIONS_KEY, prompt_path), prompt]

    returncode, answer, session, turn_failed, stderr = _run(cmd, events_path)

    # No final answer is a failure — a run that exits zero but produced nothing
    # usable is not a success, the same as a process failure or a failed turn.
    failed = returncode != 0 or turn_failed or answer is None
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
    print("session: %s" % session)
    print("output:  %s" % answer_path)
    print("events:  %s" % events_path)

    return 1 if failed else 0


# --- entrypoint -------------------------------------------------------------------

_USAGE = ('Usage:\n'
          '  codex-run @<agent> "<prompt>"        run codex as <agent>\n'
          '  codex-run resume <session> "<msg>"   continue a prior run')


def main(argv):
    # Every message — errors included — goes to stdout so the result reads
    # cleanly with no downstream parsing.
    if len(argv) >= 1 and argv[0] == "resume":
        if len(argv) != 3:
            print("codex-run: resume needs a session id and a message\n" + _USAGE)
            return 2
        return _dispatch(None, argv[2], resume_id=argv[1])

    if len(argv) != 2 or not argv[0].startswith("@"):
        print(_USAGE)
        return 2

    prompt_path = _resolve_agent(argv[0])
    if prompt_path is None:
        print("codex-run: unknown agent '%s'. Available: %s"
              % (argv[0], ", ".join(_available_agents())))
        return 1
    return _dispatch(prompt_path, argv[1])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
