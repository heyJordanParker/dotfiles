"""Behavioral gate for the two project-docs injectors.

inject_docs.py and inject_rules.py each own a single job; this pins each to its
own events so a regression that bleeds one into the other fails the suite, not
production:

- inject_docs.py fires ONLY on the Bash trace-command path. It loads the trace
  target's docs and blocks (exit 2) when `trace docs` fails, so the agent never
  traces without project-docs context. SessionStart and file touches are no-ops.
- inject_rules.py owns Codex rule injection: SessionStart loads the repo-root
  rules, a file touch (Read/Write/Edit/apply_patch) loads the touched file's
  rules. It never handles the Bash trace-command path.

Both injectors emit project docs as a hookSpecificOutput.additionalContext
envelope wrapping `trace docs --json`; we assert on the envelope and exit code.
Skips when `trace` isn't on PATH (each hook's own missing-binary no-op).

Cases call main() in this process. Two per hook stay a real `python3 <hook>`
run, because the harness reads the exit code and the stdout envelope off the
process: for inject_docs the emit and the block, for inject_rules the
SessionStart emit and the Bash no-op.
"""

import io
import json
import os
import re
import shutil
import subprocess
import sys
import time

import inject_docs
import pytest
from conftest import PY_HOOKS, REPO

DOCS = os.path.join(PY_HOOKS, "inject_docs.py")
RULES = os.path.join(PY_HOOKS, "inject_rules.py")

# `trace docs` dedupes per (session id, doc path): a doc already loaded under a
# session id returns doc_count 0 on the next call. The emit assertions need a
# first-load, so every session id is salted unique-per-run — the same move the
# enrich-on-read suite uses (fresh session id per case).
RUN = f"{os.getpid()}-{int(time.time() * 1000)}"


def _sid(name):
    return f"{name}-{RUN}"

# A repo file with a governing Claude.md, so `trace docs` returns a non-zero
# doc_count and the hook emits.
TARGET_FILE = os.path.join(PY_HOOKS, "inject_docs.py")

pytestmark = pytest.mark.skipif(shutil.which("trace") is None, reason="trace binary not on PATH")


def _run(hook, payload, env=None):
    r = subprocess.run(["python3", hook], input=json.dumps(payload), text=True,
                       capture_output=True, env=env)
    return r.returncode, r.stdout.strip(), r.stderr.strip()


def _call(module, payload, monkeypatch, capsys):
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    rc = module.main()
    captured = capsys.readouterr()
    return rc, captured.out.strip(), captured.err.strip()


def _context(stdout):
    """The additionalContext payload, unwrapped from its <name_agent> attribution
    tag, or '' when the hook emitted nothing."""
    if not stdout:
        return ""
    ctx = json.loads(stdout)["hookSpecificOutput"]["additionalContext"]
    return re.sub(r"^<\w+_agent>\n(.*)\n</\w+_agent>$", r"\1", ctx, flags=re.S)


def _event_name(stdout):
    return json.loads(stdout)["hookSpecificOutput"]["hookEventName"]


# --- inject_docs: Bash trace-command guard ----------------------------------

def test_docs_emits_for_path_taking_trace_command():
    """A path-taking `trace read <file>` loads that file's docs and injects them."""
    rc, out, _ = _run(DOCS, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": _sid("docs-emit"), "agent_id": "a",
    })
    assert rc == 0
    ctx = _context(out)
    assert ctx, "expected injected docs for a path-taking trace command"
    assert json.loads(ctx).get("doc_count", 0) > 0


def test_docs_blocks_when_trace_docs_fails(tmp_path):
    binv = tmp_path / "bin"
    binv.mkdir()
    stub = binv / "trace"
    stub.write_text("#!/bin/bash\necho 'boom' >&2\nexit 1\n")
    stub.chmod(0o755)
    env = dict(os.environ, PATH=f"{binv}:{os.environ['PATH']}")
    rc, out, err = _run(DOCS, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": "docs-block", "agent_id": "a",
    }, env=env)
    assert rc == 2
    assert out == ""
    assert "BLOCKED: project-docs load failed" in err




def test_docs_ignores_session_start(monkeypatch, capsys):
    """SessionStart is not inject_docs' job — no command, clean no-op."""
    rc, out, err = _call(inject_docs, {
        "hook_event_name": "SessionStart", "source": "startup",
        "cwd": REPO, "session_id": "docs-ss", "agent_id": "a",
    }, monkeypatch, capsys)
    assert rc == 0 and out == "" and err == ""








# --- inject_rules: Codex rule injection -------------------------------------

def test_rules_emits_on_session_start():
    """SessionStart loads the repo-root rules, wrapped as a SessionStart envelope."""
    rc, out, _ = _run(RULES, {
        "hook_event_name": "SessionStart", "source": "startup",
        "cwd": REPO, "session_id": _sid("rules-ss"), "agent_id": "a",
    })
    assert rc == 0
    ctx = _context(out)
    assert ctx, "expected repo-root rules on SessionStart"
    assert json.loads(ctx).get("doc_count", 0) > 0
    assert _event_name(out) == "SessionStart"






def test_rules_ignores_bash_trace_command():
    """inject_rules does NOT handle the Bash trace-command path — that's inject_docs'
    job. A `trace read` Bash event is a clean no-op."""
    rc, out, err = _run(RULES, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": "rules-bash", "agent_id": "a",
    })
    assert rc == 0 and out == "" and err == ""
