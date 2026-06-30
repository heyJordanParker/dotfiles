"""Deterministic machinery of the stop-validation judge (validate_completion.py).

The judge's verdict is the live model's call and is not unit-tested. What IS
pinned here is everything deterministic the model is handed: the per-edited-file
read-coverage and caller-read facts, the opened-files set, and the prompt that
carries the two blind-edit rules. The pure helpers are tested in isolation; the
fact-gathering helpers run against the real tracer (skipped when it is absent),
recording reads the same way the enrich hook does — through `trace context`.
"""

import os
import shutil
import subprocess

import pytest
import validate_completion as v
from conftest import REPO

# --- pure: per-edited-file fact line + blind-edit risk ----------------------

def test_edit_fact_line_barely_read_is_risk():
    line, risk = v._edit_fact_line("/r/foo.py", 0.1, [], set())
    assert line == "- /r/foo.py: 10% read; no caller files in the graph"
    assert risk is True


def test_edit_fact_line_no_read_recorded_is_risk():
    line, risk = v._edit_fact_line("/r/foo.py", None, [], set())
    assert "no read recorded" in line
    assert risk is True


def test_edit_fact_line_unread_callers_is_risk():
    # Fully read, but none of the two caller files appear in the opened set.
    line, risk = v._edit_fact_line(
        "/r/foo.py", 1.0, ["pkg/a.py", "pkg/b.py"], {"/x/unrelated.py"})
    assert line == "- /r/foo.py: 100% read; 2 caller file(s), 0 read"
    assert risk is True


def test_edit_fact_line_read_with_a_read_caller_is_safe():
    # A caller counts as read when an opened path ends with its repo-relative path.
    line, risk = v._edit_fact_line(
        "/r/foo.py", 1.0, ["pkg/a.py"], {"/abs/pkg/a.py"})
    assert line == "- /r/foo.py: 100% read; 1 caller file(s), 1 read"
    assert risk is False


def test_edit_fact_line_fully_read_no_callers_is_safe():
    line, risk = v._edit_fact_line("/r/foo.py", 1.0, [], set())
    assert "no caller files in the graph" in line
    assert risk is False


# --- pure: the judge-facing facts section -----------------------------------

def test_edit_facts_context_includes_both_blocks():
    ctx = v._edit_facts_context(
        "- /r/foo.py: 100% read; 1 caller file(s), 0 read",
        {"/r/foo.py", "/r/bar.py"})
    assert "Files this turn EDITED" in ctx
    assert "0 read" in ctx
    assert "Files the agent OPENED" in ctx
    assert "/r/bar.py" in ctx and "/r/foo.py" in ctx


def test_edit_facts_context_empty_when_nothing_to_say():
    assert v._edit_facts_context("", set()) == ""


def test_edit_facts_context_opened_only_when_no_edits():
    # A pure proposing turn: no edits, but the opened set still drives rule 12.
    ctx = v._edit_facts_context("", {"/r/seen.py"})
    assert "Files this turn EDITED" not in ctx
    assert "Files the agent OPENED" in ctx and "/r/seen.py" in ctx


# --- pure: the prompt carries both new rules and the facts ------------------

def test_eval_prompt_carries_both_blind_edit_rules_and_facts():
    facts = v._edit_facts_context(
        "- /r/foo.py: 30% read; 2 caller file(s), 0 read", {"/r/foo.py"})
    prompt = v._eval_prompt("", "", "user msg", "last msg", "", facts)
    assert "11. EDITED-BLIND" in prompt
    assert "12. PROPOSED-EDIT-UNOPENED" in prompt
    assert "2 caller file(s), 0 read" in prompt   # facts are embedded
    assert "Files the agent OPENED" in prompt


# --- live: fact-gathering against the real tracer ---------------------------
#
# Reads are recorded through `trace context` exactly as enrich_on_read.py records
# them in a session. Each test uses a fresh session id so coverage and the
# opened set are isolated. The architecture cache is cleared first so the caller
# lookup exercises the dirty-tree warm path, not a pre-warmed graph.

pytestmark_trace = pytest.mark.skipif(
    shutil.which("trace") is None, reason="trace binary not on PATH")

# A repo file with known direct callers in the graph (its importers).
CALLED_FILE = "packages/agents/hooks/lib/transcript.py"
TRANSCRIPT_CALLER = "packages/agents/hooks/classify_intent.py"


def _record_read(session_id, rel_path, offset=None, limit=None):
    """Record a read of a repo file into the session log via `trace context`,
    the same surface the enrich hook drives."""
    env = v._trace_env(session_id)
    args = ["trace", "context", rel_path]
    if offset is not None:
        args += ["--offset", str(offset), "--limit", str(limit)]
    subprocess.run(args, cwd=REPO, env=env, capture_output=True)


@pytestmark_trace
def test_read_log_status_reports_coverage_and_opened_set():
    sid = "vc-status-%d" % os.getpid()
    _record_read(sid, CALLED_FILE)                       # whole file
    _record_read(sid, "packages/agents/hooks/lib/event.py", offset=1, limit=3)  # partial

    coverage, opened = v._read_log_status(REPO, v._trace_env(sid))

    whole = os.path.realpath(os.path.join(REPO, CALLED_FILE))
    partial = os.path.realpath(os.path.join(REPO, "packages/agents/hooks/lib/event.py"))
    assert whole in opened and partial in opened
    assert coverage[whole] == 1.0
    assert 0.0 < coverage[partial] < 1.0


@pytestmark_trace
def test_edited_facts_flags_unread_callers_then_clears_when_caller_read():
    # Stale the graph so _edited_facts must warm it before the caller lookup.
    subprocess.run(["trace", "cache", "clear", "--namespace", "architecture"],
                   cwd=REPO, capture_output=True)
    sid = "vc-callers-%d" % os.getpid()
    _record_read(sid, CALLED_FILE)                       # read the file, none of its callers
    coverage, opened = v._read_log_status(REPO, v._trace_env(sid))
    block, risk = v._edited_facts([CALLED_FILE], coverage, opened, REPO, v._trace_env(sid))
    assert "caller file(s), 0 read" in block
    assert risk is True

    _record_read(sid, TRANSCRIPT_CALLER)                 # now read one caller
    coverage, opened = v._read_log_status(REPO, v._trace_env(sid))
    block, risk = v._edited_facts([CALLED_FILE], coverage, opened, REPO, v._trace_env(sid))
    assert "caller file(s), 1 read" in block
    assert risk is False


@pytestmark_trace
def test_edited_facts_flags_an_unread_edited_file():
    # A file the session never read at all → no coverage entry → barely-read risk.
    sid = "vc-unread-%d" % os.getpid()
    coverage, opened = v._read_log_status(REPO, v._trace_env(sid))
    block, risk = v._edited_facts([CALLED_FILE], coverage, opened, REPO, v._trace_env(sid))
    assert "no read recorded" in block
    assert risk is True
