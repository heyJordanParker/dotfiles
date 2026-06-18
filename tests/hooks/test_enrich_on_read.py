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
        "__init__.py", "codex_run.py", "command.py", "event.py",
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
