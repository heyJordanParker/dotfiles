#!/usr/bin/env python3
"""session-state — unified Claude session state helper.

All hooks read/write per-session state through this module. Invoke as:
python3 state.py <command> [args...]

Storage root: $CLAUDE_DATA_ROOT or ~/.claude
Sessions live at: <root>/sessions/<session_id>/
Subagents nest at: <root>/sessions/<parent_id>/subagents/<agent_id>/
Schema version: 1

The clock and ISO timestamp shell out to `date` so the test harness's
PATH-level `date` mock can drive time deterministically.
"""

import json
import os
import re
import subprocess
import sys
import tempfile
import time
from glob import glob

_SID_RE = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_-]*$")


# ============================================================================
# Private helpers
# ============================================================================

def _data_root():
    return os.environ.get("CLAUDE_DATA_ROOT") or os.path.join(os.path.expanduser("~"), ".claude")


def _sessions_root():
    return os.path.join(_data_root(), "sessions")


def _projects_root():
    return os.environ.get("CLAUDE_PROJECTS_ROOT") or os.path.join(os.path.expanduser("~"), ".claude", "projects")


def _now():
    # Shell out so the harness's `date` mock (PATH override) drives time.
    out = subprocess.run(["date", "+%s"], capture_output=True, text=True)
    return int(out.stdout.strip())


def _err(msg):
    sys.stderr.write(msg + "\n")


def _read_json(path):
    try:
        with open(path, "r", encoding="utf-8") as fh:
            return json.load(fh)
    except Exception:
        return None


def _dump(obj):
    return json.dumps(obj, separators=(",", ":"))


def _atomic_write(path, content):
    d = os.path.dirname(path)
    try:
        os.makedirs(d, exist_ok=True)
    except Exception:
        return False
    try:
        fd, tmp = tempfile.mkstemp(prefix=".session-state.", dir=d)
    except Exception:
        return False
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(content + "\n")
        os.replace(tmp, path)
        return True
    except Exception:
        try:
            os.unlink(tmp)
        except Exception:
            pass
        return False


def _with_lock(path):
    lock_dir = path + ".lock"
    tries = 0
    while True:
        try:
            os.mkdir(lock_dir)
            return True
        except FileExistsError:
            tries += 1
            if tries > 500:
                return False
            time.sleep(0.01)
        except Exception:
            tries += 1
            if tries > 500:
                return False
            time.sleep(0.01)


def _release_lock(path):
    try:
        os.rmdir(path + ".lock")
    except Exception:
        pass


def _human_prompt(content):
    """True if human-typed, False if system-injected."""
    if content == "":
        return False

    # Rule 1: XML-tagged with a matching close tag
    if content.startswith("<"):
        rest = content[1:]
        tag = re.split(r"[> ]", rest, maxsplit=1)[0]
        if tag and ("</%s>" % tag) in content:
            return False

    # Rule 2: single-line bracket-enclosed
    if "\n" not in content and content.startswith("[") and content.endswith("]"):
        return False

    # Rule 3: continuation prefix
    if content.startswith("This session is being continued"):
        return False

    # Rule 4: skill expansion prefix
    if content.startswith("Base directory for this skill:"):
        return False

    return True


def _default_main_state(session_id):
    return {
        "session_id": session_id,
        "role": "main",
        "parent_session_id": None,
        "approach": "subagents",
        "state": "proposing",
        "commit_requested": False,
        "goal": None,
        "notes": [],
        "gate_blocks": {},
        "current_turn_start": None,
        "schema_version": 1,
    }


def _default_subagent_state(session_id, parent_id=""):
    return {
        "session_id": session_id,
        "role": "subagent",
        "parent_session_id": (parent_id if parent_id else None),
        "current_turn_start": None,
        "schema_version": 1,
    }


def _is_valid_session_id(sid):
    if not sid:
        return False
    return bool(_SID_RE.match(sid))


def own_session_id():
    """The session id the current process runs under, from the environment.

    Order: AGENT_SESSION_ID (the harness-neutral carrier our own code sets, an
    explicit hook write) → CODEX_THREAD_ID → CLAUDE_CODE_SESSION_ID. The
    innermost harness session wins over the outer, so a codex run nested under a
    Claude launcher keys on its own thread, not the launcher's. The single
    own-session resolution point for our Python tooling; the tracer reads the
    same three-name order. Empty string when nothing is set."""
    return (os.environ.get("AGENT_SESSION_ID")
            or os.environ.get("CODEX_THREAD_ID")
            or os.environ.get("CLAUDE_CODE_SESSION_ID")
            or "")


def _parse_parent_from_transcript(path):
    if re.search(r"/subagents/agent-[^/]*\.jsonl$", path):
        count = path.count("/subagents/")
        if count != 1:
            return ""
        parent = os.path.basename(os.path.dirname(os.path.dirname(path)))
        if _is_valid_session_id(parent):
            return parent
    return ""


def _session_dir(session_id):
    sessions_root = _sessions_root()
    main_dir = os.path.join(sessions_root, session_id)
    if os.path.isdir(main_dir):
        return main_dir
    # find -mindepth 3 -maxdepth 3 -type d -name <sid> -path */subagents/*
    pattern = os.path.join(sessions_root, "*", "subagents", session_id)
    for cand in sorted(glob(pattern)):
        if os.path.isdir(cand):
            return cand
    return None


def _resolve_parent_id(session_id):
    if not session_id.startswith("agent-"):
        return ""
    projects_root = _projects_root()
    if not os.path.isdir(projects_root):
        return ""
    matches = glob(os.path.join(projects_root, "*", "*", "subagents", session_id + ".jsonl"))
    if len(matches) != 1:
        return ""
    parent = os.path.basename(os.path.dirname(os.path.dirname(matches[0])))
    if _is_valid_session_id(parent):
        return parent
    return ""


def _ensure_session(session_id, parent_override=""):
    resolved_parent = parent_override
    if not resolved_parent:
        resolved_parent = _resolve_parent_id(session_id)

    # Case-collision check for top-level mains (case-insensitive APFS guard)
    if not resolved_parent:
        sessions_root_path = _sessions_root()
        if os.path.isdir(sessions_root_path):
            on_disk = ""
            for entry in sorted(glob(os.path.join(sessions_root_path, "*"))):
                if not os.path.isdir(entry):
                    continue
                name = os.path.basename(entry)
                if name.lower() == session_id.lower():
                    on_disk = name
                    break
            if on_disk and on_disk != session_id:
                _err("Error: session_id '%s' collides case-insensitively with existing '%s'" % (session_id, on_disk))
                return None

    # Already exists — heal corrupt/missing/empty state.json in place
    session_dir = _session_dir(session_id)
    if session_dir:
        state_file = os.path.join(session_dir, "state.json")
        needs_defaults = False
        if not os.path.isfile(state_file):
            needs_defaults = True
        elif os.path.getsize(state_file) == 0:
            needs_defaults = True
        else:
            obj = _read_json(state_file)
            if not isinstance(obj, dict):
                needs_defaults = True
        if needs_defaults:
            if "/subagents/" in session_dir:
                existing_parent = os.path.basename(os.path.dirname(os.path.dirname(session_dir)))
                default_state = _default_subagent_state(session_id, existing_parent)
            else:
                default_state = _default_main_state(session_id)
            _atomic_write(state_file, _dump(default_state))
        return session_dir

    # New session — agent-* without resolvable parent fails loud
    if session_id.startswith("agent-") and not resolved_parent:
        _err("Error: subagent session_id '%s' has no resolvable parent (transcript not found under $CLAUDE_PROJECTS_ROOT)" % session_id)
        return None

    if resolved_parent:
        session_dir = os.path.join(_sessions_root(), resolved_parent, "subagents", session_id)
    else:
        session_dir = os.path.join(_sessions_root(), session_id)

    os.makedirs(session_dir, exist_ok=True)

    state_file = os.path.join(session_dir, "state.json")
    if resolved_parent:
        default_state = _default_subagent_state(session_id, resolved_parent)
    else:
        default_state = _default_main_state(session_id)
    _atomic_write(state_file, _dump(default_state))

    return session_dir


# ============================================================================
# Public subcommands. Each returns an int exit code.
# ============================================================================

def cmd_start(args):
    if len(args) < 1:
        _err("Error: start requires session_id")
        return 1
    session_id = args[0]
    rest = args[1:]

    if not _is_valid_session_id(session_id):
        _err("Error: invalid session_id: %s" % session_id)
        return 1

    transcript_path = ""
    i = 0
    while i < len(rest):
        if rest[i] == "--transcript-path":
            if i + 1 >= len(rest):
                _err("Error: --transcript-path requires a value")
                return 1
            transcript_path = rest[i + 1]
            i += 2
        else:
            _err("Error: unknown start flag: %s" % rest[i])
            return 1

    parent_override = ""
    if transcript_path:
        parent_override = _parse_parent_from_transcript(transcript_path)

    session_dir = _ensure_session(session_id, parent_override)
    if session_dir is None:
        return 1

    if transcript_path:
        _atomic_write(os.path.join(session_dir, "transcript"), transcript_path)
    return 0


def cmd_end(args):
    if len(args) < 1:
        _err("Error: end requires session_id")
        return 1
    session_id = args[0]
    if not _is_valid_session_id(session_id):
        _err("Error: invalid session_id: %s" % session_id)
        return 1
    session_dir = _session_dir(session_id)
    if session_dir:
        # rm -rf semantics: a symlink target dir is reached by _session_dir
        # (isdir follows links), but removal must drop the link, not its target.
        if os.path.islink(session_dir):
            try:
                os.unlink(session_dir)
            except Exception:
                pass
        else:
            import shutil
            shutil.rmtree(session_dir, ignore_errors=True)
    return 0


def cmd_get(args):
    # Path-resolution mode
    if len(args) > 0 and args[0] == "--path":
        rest = args[1:]
        if len(rest) == 0:
            csid = own_session_id()
            if not csid:
                _err("Error: no current session id in environment")
                return 1
            if not _is_valid_session_id(csid):
                _err("Error: invalid session id: %s" % csid)
                return 1
            d = _session_dir(csid)
            if d:
                print(d)
                return 0
            return 1
        target = rest[0]
        if target == "data-root":
            print(_data_root())
            return 0
        if target == "sessions":
            print(_sessions_root())
            return 0
        if target == "shaping":
            print(os.path.join(_data_root(), "shaping"))
            return 0
        if not _is_valid_session_id(target):
            _err("Error: invalid path target: %s" % target)
            return 1
        d = _session_dir(target)
        if d:
            print(d)
            return 0
        return 1

    # Field-read mode
    if len(args) < 2:
        _err("Error: get requires session_id + field, or --path [target]")
        return 1
    session_id = args[0]
    field = args[1]
    if not _is_valid_session_id(session_id):
        _err("Error: invalid session_id: %s" % session_id)
        return 1
    session_dir = _session_dir(session_id)
    if not session_dir:
        return 0
    state_file = os.path.join(session_dir, "state.json")
    if not os.path.isfile(state_file):
        return 0
    state = _read_json(state_file)
    if not isinstance(state, dict):
        return 0
    val = state.get(field)
    if val is None or val is False:
        return 0  # jq '// empty' → null and false both emit nothing
    if isinstance(val, bool):
        print("true" if val else "false")
    elif isinstance(val, (dict, list)):
        print(_dump(val))
    else:
        print(val)
    return 0


def cmd_set(args):
    if len(args) < 3:
        _err("Error: set requires session_id, field, value")
        return 1
    session_id, field, value = args[0], args[1], args[2]
    if not _is_valid_session_id(session_id):
        _err("Error: invalid session_id: %s" % session_id)
        return 1
    session_dir = _ensure_session(session_id)
    if session_dir is None:
        return 1
    state_file = os.path.join(session_dir, "state.json")
    if not _with_lock(state_file):
        _err("Error: lock timeout: %s" % state_file)
        return 1
    try:
        state = _read_json(state_file)
        if not isinstance(state, dict):
            _err("Error: failed to update state.json: %s" % state_file)
            return 1
        try:
            parsed = json.loads(value)
            state[field] = parsed
        except Exception:
            state[field] = value
        if not _atomic_write(state_file, _dump(state)):
            return 1
        return 0
    finally:
        _release_lock(state_file)


def load_state(session_id):
    """The session's parsed state.json, or {} when the id is invalid or no session exists; read-only, never heals."""
    if not _is_valid_session_id(session_id):
        return {}
    session_dir = _session_dir(session_id)
    if not session_dir:
        return {}
    state = _read_json(os.path.join(session_dir, "state.json"))
    return state if isinstance(state, dict) else {}


def merge_state(session_id, fragment):
    """Lock-guarded atomic merge of a dict fragment into the session's state.json; validates the id itself since in-process callers bypass cmd_merge. True on success."""
    if not _is_valid_session_id(session_id) or not isinstance(fragment, dict):
        return False
    session_dir = _ensure_session(session_id)
    if session_dir is None:
        return False
    state_file = os.path.join(session_dir, "state.json")
    if not _with_lock(state_file):
        return False
    try:
        state = _read_json(state_file)
        if not isinstance(state, dict):
            return False
        state.update(fragment)
        return _atomic_write(state_file, _dump(state))
    finally:
        _release_lock(state_file)


def gate_block_count(session_id, gate):
    """How many times a stop gate has blocked the stop in the current turn.

    Kept per turn, keyed to current_turn_start: a fresh human turn (which advances
    current_turn_start) reads as 0 again, so a gate flags an issue once per turn,
    then yields. Pure read."""
    state = load_state(session_id)
    rec = (state.get("gate_blocks") or {}).get(gate) or {}
    if rec.get("turn") != state.get("current_turn_start"):
        return 0
    return rec.get("count") or 0


def bump_gate_block(session_id, gate):
    """Record one stop-gate block for the current turn and return the new count.

    Lock-guarded read-modify-write so the two stop gates blocking on the same Stop
    don't clobber each other's counts in the shared gate_blocks map."""
    if not _is_valid_session_id(session_id):
        return 0
    session_dir = _ensure_session(session_id)
    if session_dir is None:
        return 0
    state_file = os.path.join(session_dir, "state.json")
    if not _with_lock(state_file):
        return 0
    try:
        state = _read_json(state_file)
        if not isinstance(state, dict):
            return 0
        turn = state.get("current_turn_start")
        blocks = state.get("gate_blocks") or {}
        rec = blocks.get(gate) or {}
        count = ((rec.get("count") or 0) if rec.get("turn") == turn else 0) + 1
        blocks[gate] = {"turn": turn, "count": count}
        state["gate_blocks"] = blocks
        _atomic_write(state_file, _dump(state))
        return count
    finally:
        _release_lock(state_file)


def cmd_merge(args):
    if len(args) < 2:
        _err("Error: merge requires session_id and json_fragment")
        return 1
    session_id, fragment = args[0], args[1]
    if not _is_valid_session_id(session_id):
        _err("Error: invalid session_id: %s" % session_id)
        return 1
    try:
        frag = json.loads(fragment)
    except Exception:
        frag = None
    if not isinstance(frag, dict):
        _err("Error: merge fragment must be a JSON object: %s" % fragment)
        return 1
    if not merge_state(session_id, frag):
        _err("Error: failed to merge into state.json for %s" % session_id)
        return 1
    return 0


def cmd_prompt(args):
    if len(args) < 1:
        _err("Error: prompt requires session_id")
        return 1
    session_id = args[0]
    if not _is_valid_session_id(session_id):
        _err("Error: invalid session_id: %s" % session_id)
        return 1
    content = sys.stdin.read()
    if not _human_prompt(content):
        return 0
    session_dir = _ensure_session(session_id)
    if session_dir is None:
        return 1
    state_file = os.path.join(session_dir, "state.json")
    if not _with_lock(state_file):
        _err("Error: lock timeout: %s" % state_file)
        return 1
    try:
        state = _read_json(state_file)
        if not isinstance(state, dict):
            _err("Error: failed to record prompt event: %s" % state_file)
            return 1
        state["current_turn_start"] = _now()
        if not _atomic_write(state_file, _dump(state)):
            return 1
        return 0
    finally:
        _release_lock(state_file)


_DISPATCH = {
    "start": cmd_start,
    "end": cmd_end,
    "get": cmd_get,
    "set": cmd_set,
    "merge": cmd_merge,
    "prompt": cmd_prompt,
}


def main(argv):
    if len(argv) == 0:
        _err("Usage: session-state <command> [args...]")
        return 1
    cmd = argv[0]
    handler = _DISPATCH.get(cmd)
    if handler is None:
        _err("Error: unknown command: %s" % cmd)
        return 1
    return handler(argv[1:])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
