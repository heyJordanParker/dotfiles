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
"""

import json
import os
import re
import shutil
import subprocess
import time

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
    """The Bash trace-command path blocks (exit 2) with the BLOCKED message when
    `trace docs` fails — the agent must not trace without project-docs context.

    Shadow `trace` with a stub that exits non-zero (front of PATH), so the hook's
    `which("trace")` still resolves but the `trace docs` subprocess fails."""
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
    assert rc == 2, f"expected block (exit 2), got {rc}"
    assert out == ""
    assert "BLOCKED: project-docs load failed" in err


def test_docs_ignores_session_start():
    """SessionStart is not inject_docs' job — no command, clean no-op."""
    rc, out, err = _run(DOCS, {
        "hook_event_name": "SessionStart", "source": "startup",
        "cwd": REPO, "session_id": "docs-ss", "agent_id": "a",
    })
    assert rc == 0 and out == "" and err == ""


def test_docs_ignores_file_touch():
    """A Read/Write/Edit file touch is not inject_docs' job — no command,
    clean no-op (no Bash tool_input.command present)."""
    for tool in ("Read", "Write", "Edit"):
        rc, out, err = _run(DOCS, {
            "tool_name": tool,
            "tool_input": {"file_path": TARGET_FILE},
            "cwd": REPO, "session_id": f"docs-file-{tool}", "agent_id": "a",
        })
        assert rc == 0 and out == "" and err == "", f"{tool}: expected no-op"


def test_docs_ignores_apply_patch():
    """An apply_patch is not inject_docs' job — clean no-op."""
    rc, out, err = _run(DOCS, {
        "tool_name": "apply_patch",
        "tool_input": {"input": f"*** Update File: {TARGET_FILE}\n"},
        "cwd": REPO, "session_id": "docs-patch", "agent_id": "a",
    })
    assert rc == 0 and out == "" and err == ""


def test_docs_ignores_non_path_taking_trace():
    """A non-path-taking trace subcommand (e.g. `trace status`) is a clean no-op."""
    rc, out, err = _run(DOCS, {
        "tool_name": "Bash",
        "tool_input": {"command": "trace status"},
        "cwd": REPO, "session_id": "docs-status", "agent_id": "a",
    })
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


def test_rules_emits_on_file_touch():
    """A file touch loads the touched file's rules, wrapped as a PreToolUse envelope."""
    for tool in ("Read", "Write", "Edit"):
        rc, out, _ = _run(RULES, {
            "tool_name": tool,
            "tool_input": {"file_path": TARGET_FILE},
            "cwd": REPO, "session_id": _sid(f"rules-file-{tool}"), "agent_id": "a",
        })
        assert rc == 0, f"{tool}: exit {rc}"
        ctx = _context(out)
        assert ctx, f"{tool}: expected touched-file rules"
        assert json.loads(ctx).get("doc_count", 0) > 0
        assert _event_name(out) == "PreToolUse"


def test_rules_emits_on_apply_patch():
    """apply_patch resolves its target file and loads that file's rules."""
    rc, out, _ = _run(RULES, {
        "tool_name": "apply_patch",
        "tool_input": {"input": f"*** Update File: {TARGET_FILE}\n@@\n-x\n+y\n"},
        "cwd": REPO, "session_id": _sid("rules-patch"), "agent_id": "a",
    })
    assert rc == 0
    ctx = _context(out)
    assert ctx, "expected touched-file rules for apply_patch"
    assert json.loads(ctx).get("doc_count", 0) > 0


def test_rules_ignores_bash_trace_command():
    """inject_rules does NOT handle the Bash trace-command path — that's inject_docs'
    job. A `trace read` Bash event is a clean no-op."""
    rc, out, err = _run(RULES, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": "rules-bash", "agent_id": "a",
    })
    assert rc == 0 and out == "" and err == ""
