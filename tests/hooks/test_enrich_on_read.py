"""Behavioral gate for the tracer enrich hook (enrich_on_read.py).

The hook injects the full `trace context` shoulder before the file-touch tools
the agent uses. This gate pins which tool types get a shoulder and over which
files, so a dropped branch (a lost Edit/Write/Grep enrichment, or a Glob degraded
to the thin `--details` shoulder) fails the suite instead of hiding.

Assertions are on the SET of enriched file paths and the shoulder count, not raw
stdout: `trace context` dedupes its `[symbols]`/`[dir]` lines against the
per-session log, so two runs sharing a session id diverge by run order. Each case
therefore gets its own fresh session id; we assert which files got a shoulder —
the property that distinguishes "this tool is enriched" from "this tool is a
no-op".

Skips when `trace` isn't on PATH (the hook's own missing-binary no-op).
"""

import json
import os
import re
import shutil
import subprocess

import pytest
from conftest import PY_HOOKS, REPO

PY = os.path.join(PY_HOOKS, "enrich_on_read.py")

# A directory with several tracked files for the multi-file tools to match over.
LIB = os.path.join(PY_HOOKS, "lib")

# Every .py the multi-file tools (Glob lib/*.py, Grep "import" over lib/) resolve
# to — the exact enriched-file set the hook produces for those scenarios.
LIB_PY_FILES = {
    os.path.join(LIB, name) for name in (
        "__init__.py", "codex_run.py", "command.py", "event.py", "feedback.py",
        "model_call.py", "session_state.py", "transcript.py",
    )
}

pytestmark = pytest.mark.skipif(shutil.which("trace") is None, reason="trace binary not on PATH")


def _run(payload):
    r = subprocess.run(["python3", PY], input=json.dumps(payload), text=True, capture_output=True)
    return r.returncode, r.stdout.strip(), r.stderr.strip()


def _context(stdout):
    """The additionalContext string the hook emitted, or '' when it emitted nothing."""
    if not stdout:
        return ""
    return json.loads(stdout)["hookSpecificOutput"]["additionalContext"]


def _enriched_files(context):
    """Absolute path headers that received a shoulder (multi-file tools only).

    Multi-file tools (Glob/Grep) prefix each per-file shoulder with its path;
    single-file tools (Read/Edit/Write) emit a bare headerless shoulder, so this
    is empty for them — use _shoulder_count for those.
    """
    return set(re.findall(r"^(/\S+)$", context, re.M))


def _shoulder_count(context):
    """How many file shoulders the output carries — one `[git: ...]` line each.

    Works for both single-file (one shoulder, no path header) and multi-file
    (one shoulder per matched file) output, so it distinguishes "enriched" from
    "no-op" for every tool — including Read/Edit/Write, where the headerless
    shoulder leaves _enriched_files empty.
    """
    return len(re.findall(r"^\[git:", context, re.M))


def _enrich(payload):
    rc, out, err = _run(payload)
    return rc, _context(out), err


# --- single-file tools: one bare headerless shoulder for the target file --------

@pytest.mark.parametrize("tool", ["Read", "Edit", "Write"])
def test_single_file_tool_emits_one_headerless_shoulder(tool):
    """Read/Edit/Write each enrich their target with exactly one shoulder.

    The shoulder is headerless (no path line), so _enriched_files is empty; the
    shoulder count is what proves the tool branch is live. Fails if the branch is
    dropped — the removed tool emits no shoulder.
    """
    rc, ctx, _ = _enrich({"tool_name": tool,
                          "tool_input": {"file_path": os.path.join(LIB, "event.py")},
                          "session_id": "single-%s" % tool, "agent_id": "a"})
    assert rc == 0
    assert _shoulder_count(ctx) == 1, f"{tool}: {_shoulder_count(ctx)} shoulders, expected 1"
    assert _enriched_files(ctx) == set(), f"{tool}: single-file shoulder must be headerless"


# --- multi-file tools: one full shoulder per matched file -----------------------

def test_glob_enriches_each_matched_file():
    """Glob enriches every file its pattern matches, one shoulder each."""
    rc, ctx, _ = _enrich({"tool_name": "Glob",
                         "tool_input": {"pattern": "lib/*.py", "path": PY_HOOKS},
                         "session_id": "glob-lib", "agent_id": "a"})
    assert rc == 0
    assert _enriched_files(ctx) == LIB_PY_FILES
    assert _shoulder_count(ctx) == len(LIB_PY_FILES)


def test_grep_enriches_each_file_with_a_match():
    """Grep enriches every distinct file containing a match, one shoulder each."""
    rc, ctx, _ = _enrich({"tool_name": "Grep",
                         "tool_input": {"pattern": "import", "path": LIB},
                         "session_id": "grep-lib", "agent_id": "a"})
    assert rc == 0
    assert _enriched_files(ctx) == LIB_PY_FILES
    assert _shoulder_count(ctx) == len(LIB_PY_FILES)


def test_glob_emits_full_shoulder_not_details():
    """Glob carries the full `trace context` shoulder, not the thin `--details`.

    The full shoulder includes the `[docs: ...]` awareness line that `glob
    --details` never emits; its presence proves the hook enriches per-file with
    `trace context`, not with the degraded `--details` path.
    """
    _, ctx, _ = _enrich({"tool_name": "Glob",
                        "tool_input": {"pattern": "lib/*.py", "path": PY_HOOKS},
                        "session_id": "glob-shape", "agent_id": "a"})
    assert "[docs:" in ctx, f"glob shoulder missing docs line: {ctx!r}"


def test_match_cap_bounds_enriched_files():
    """Multi-file enrichment is capped at 20 files."""
    _, ctx, _ = _enrich({"tool_name": "Glob",
                        "tool_input": {"pattern": "packages/agents/hooks/**/*.py", "path": REPO},
                        "session_id": "cap", "agent_id": "a"})
    assert len(_enriched_files(ctx)) == 20, f"enriched {len(_enriched_files(ctx))}, expected cap 20"


# --- Codex-only Bash branch: no native Read tool, reads arrive as shell ---------

def test_codex_bash_read_branch_fires():
    """The Codex-only Bash shell-read branch emits a shoulder for a repo file read.

    Codex has no native Read tool; it reads through its shell tool, which arrives
    as tool_name "Bash". A `cat <repo file>` carries the same shoulder a native
    Read would.
    """
    rc, ctx, _ = _enrich({"tool_name": "Bash",
                         "tool_input": {"command": "cat %s" % os.path.join(LIB, "event.py")},
                         "session_id": "codex-read", "agent_id": "a"})
    assert rc == 0
    assert "[git:" in ctx, "Codex Bash read branch emitted no shoulder"


def test_codex_bash_nonread_is_silent():
    """A non-read Bash command yields no shoulder (exit 0, empty)."""
    rc, out, _ = _run({"tool_name": "Bash", "tool_input": {"command": "ls -la"},
                       "session_id": "codex-ls"})
    assert rc == 0 and out == ""


# --- read-coverage: the hook forwards the read's line range to the tracer -------
#
# The recorded coverage is a side effect of the `trace context` the hook runs;
# `trace docs status` is the surface that reports, per file, the fraction of the
# file's lines read this session. These pin two contracts: a native Read's
# offset/limit is forwarded (a partial read records less than full), and a shell
# read (no parsed range → whole file) is counted identically to a native
# whole-file read.

EVENT_PY = os.path.join(LIB, "event.py")


def _status_fraction(session_id, agent_id, filename):
    """The `read_fraction` `trace docs status` reports for the loaded entry
    whose path ends with `filename`, or None when it is absent."""
    env = dict(os.environ)
    env["AGENT_SESSION_ID"] = session_id
    env["TRACER_AGENT_ID"] = agent_id
    r = subprocess.run(
        ["trace", "docs", "status", "--json"],
        cwd=REPO, env=env, text=True, capture_output=True,
    )
    for entry in json.loads(r.stdout).get("loaded", []):
        if entry.get("path", "").endswith(filename):
            return entry.get("read_fraction")
    return None


def test_read_branch_forwards_offset_and_limit():
    """A partial native Read records less than full coverage; a whole one full."""
    _enrich({"tool_name": "Read",
             "tool_input": {"file_path": EVENT_PY, "offset": 1, "limit": 5},
             "session_id": "cov-partial", "agent_id": "a"})
    _enrich({"tool_name": "Read",
             "tool_input": {"file_path": EVENT_PY},
             "session_id": "cov-full", "agent_id": "a"})

    partial = _status_fraction("cov-partial", "a", "event.py")
    full = _status_fraction("cov-full", "a", "event.py")
    assert partial is not None and full is not None, "read file must appear in the manifest"
    assert full == 1.0, f"whole-file Read must report 1.0, got {full}"
    assert 0.0 < partial < full, f"partial Read must forward offset/limit: {partial} vs {full}"


def test_shell_read_counted_identically_to_native_read():
    """A shell `cat <file>` records the same coverage as a native whole-file Read."""
    _enrich({"tool_name": "Bash",
             "tool_input": {"command": "cat %s" % EVENT_PY},
             "session_id": "cov-shell", "agent_id": "a"})
    _enrich({"tool_name": "Read",
             "tool_input": {"file_path": EVENT_PY},
             "session_id": "cov-native", "agent_id": "a"})

    shell = _status_fraction("cov-shell", "a", "event.py")
    native = _status_fraction("cov-native", "a", "event.py")
    assert shell == native == 1.0, f"shell vs native coverage diverged: {shell} vs {native}"


def test_partial_shell_read_records_only_the_shown_span():
    """A `head -n N` / `sed -n 'A,Bp'` shell read records only the portion shown.

    Pre-fix, every shell read recorded a whole-file read, so a `head -n 5` of a
    100-line file reported 100% coverage. The hook now parses the range the shell
    command expresses and forwards it, so a partial shell read records less than
    full — distinct from a `cat` whole-file read at 1.0.
    """
    _enrich({"tool_name": "Bash",
             "tool_input": {"command": "head -n 5 %s" % EVENT_PY},
             "session_id": "cov-head", "agent_id": "a"})
    _enrich({"tool_name": "Bash",
             "tool_input": {"command": "sed -n '10,20p' %s" % EVENT_PY},
             "session_id": "cov-sed", "agent_id": "a"})

    head = _status_fraction("cov-head", "a", "event.py")
    sed = _status_fraction("cov-sed", "a", "event.py")
    assert head is not None and 0.0 < head < 1.0, \
        f"head -n 5 must record only the shown span, not the whole file: {head}"
    assert sed is not None and 0.0 < sed < 1.0, \
        f"sed -n '10,20p' must record only the shown span, not the whole file: {sed}"


# --- Edit/Write get the shoulder but record no read -----------------------------
#
# An edit touches a file but is not a read of it. Pre-fix, the enrich hook's
# `trace context <file>` recorded a whole-file read for an Edit/Write, so a
# partial-read-then-edit reported 100% coverage — fiction. The hook now passes
# --no-record for Edit/Write: the shoulder still renders, but the file records no
# read and its coverage reflects only genuine reads.

@pytest.mark.parametrize("tool", ["Edit", "Write"])
def test_edit_and_write_record_no_read(tool):
    """Edit/Write enrich their target but leave read coverage untouched."""
    sid = "no-record-%s" % tool
    rc, ctx, _ = _enrich({"tool_name": tool,
                          "tool_input": {"file_path": EVENT_PY},
                          "session_id": sid, "agent_id": "a"})
    assert rc == 0
    # The shoulder still renders — an edit does not lose file context.
    assert _shoulder_count(ctx) == 1, f"{tool} must still emit the file shoulder"
    # But no read was recorded: the file is absent from the session manifest.
    assert _status_fraction(sid, "a", "event.py") is None, \
        f"{tool} must record no read — the file must not appear in the manifest"


def test_partial_read_then_edit_keeps_the_partial_coverage():
    """An edit after a partial read does not inflate coverage to a full read.

    This is the bug the fix targets: a 5-line read of event.py followed by an
    Edit must keep the partial fraction, not jump to 1.0.
    """
    sid = "partial-then-edit"
    _enrich({"tool_name": "Read",
             "tool_input": {"file_path": EVENT_PY, "offset": 1, "limit": 5},
             "session_id": sid, "agent_id": "a"})
    _enrich({"tool_name": "Edit",
             "tool_input": {"file_path": EVENT_PY},
             "session_id": sid, "agent_id": "a"})

    frac = _status_fraction(sid, "a", "event.py")
    assert frac is not None and 0.0 < frac < 1.0, \
        f"the edit must not inflate the partial read to full coverage: {frac}"


# --- Glob/Grep matches record no read --------------------------------------
#
# A Glob listing or a Grep match surfaces a file's path (and, for grep, one
# matching line) — the agent never opened the file's content. Pre-fix, the
# per-match `trace context <file>` recorded a whole-file read for every matched
# file, so a grep over a file the agent never read reported 100% coverage —
# fiction that lets a blind edit slip past the read-coverage check. The hook now
# passes --no-record on the matched-file shoulders: the shoulder still renders,
# but a match records no read.


def test_glob_match_records_no_read():
    """A Glob match enriches the file but leaves its read coverage at zero."""
    sid = "glob-no-record"
    rc, ctx, _ = _enrich({"tool_name": "Glob",
                          "tool_input": {"pattern": "lib/event.py", "path": PY_HOOKS},
                          "session_id": sid, "agent_id": "a"})
    assert rc == 0
    # The shoulder still renders — a match does not lose file context.
    assert EVENT_PY in _enriched_files(ctx), "glob must still enrich the matched file"
    # But no read was recorded: the file is absent from the session manifest.
    assert _status_fraction(sid, "a", "event.py") is None, \
        "a glob match must record no read — the matched file must not appear in the manifest"


def test_grep_match_records_no_read():
    """A Grep match enriches the file but leaves its read coverage at zero."""
    sid = "grep-no-record"
    rc, ctx, _ = _enrich({"tool_name": "Grep",
                          "tool_input": {"pattern": "import", "path": EVENT_PY},
                          "session_id": sid, "agent_id": "a"})
    assert rc == 0
    assert EVENT_PY in _enriched_files(ctx), "grep must still enrich the matched file"
    assert _status_fraction(sid, "a", "event.py") is None, \
        "a grep match must record no read — the matched file must not appear in the manifest"


@pytest.mark.parametrize("payload", [
    {"tool_name": "Read", "tool_input": {}},          # missing file_path
    {"tool_name": "Glob", "tool_input": {}},          # missing pattern
    {"tool_name": "Grep", "tool_input": {}},          # missing pattern
    {"tool_name": "WebFetch", "tool_input": {}},      # unhandled tool
])
def test_silent_fallback_exits_zero_no_output(payload):
    """Every degenerate input exits 0 with no stdout — the hook never blocks."""
    rc, out, _ = _run({**payload, "session_id": "fallback"})
    assert rc == 0 and out == ""
