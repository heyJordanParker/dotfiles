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
envelope wrapping `trace docs` Markdown, one `## <path>` section per doc; we
assert on the envelope and exit code. reload_harness_context records what
Claude Code reports loading, so neither injector sends it again.
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
RELOAD = os.path.join(PY_HOOKS, "reload_harness_context.py")

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


def _docs_surfaced(ctx):
    """How many docs the injection sent: one `## <path>` heading each."""
    return len(re.findall(r"^## \S", ctx, flags=re.M))


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
    assert _docs_surfaced(ctx) > 0


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
    assert _docs_surfaced(ctx) > 0
    assert _event_name(out) == "SessionStart"






def test_docs_injection_fits_one_hook_message():
    """Claude Code cuts additionalContext above 10,000 characters to a preview, so
    the whole injection — tag included — stays within it."""
    rc, out, _ = _run(RULES, {
        "hook_event_name": "SessionStart", "source": "startup",
        "cwd": REPO, "session_id": _sid("rules-fit"), "agent_id": "a",
    })
    assert rc == 0
    wrapped = json.loads(out)["hookSpecificOutput"]["additionalContext"]
    assert len(wrapped) <= 10_000, len(wrapped)


def test_docs_takes_the_searched_path_for_grep():
    """`trace grep <pattern> <path>` gets the searched path's docs, not the
    working directory's: the pattern is not a path."""
    hooks_dir = os.path.join(REPO, "packages", "agents", "hooks")
    rc, out, _ = _run(DOCS, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace grep needle {hooks_dir}"},
        "cwd": REPO, "session_id": _sid("docs-grep"), "agent_id": "a",
    })
    assert rc == 0
    # The searched path's doc, which a working-directory lookup never reaches.
    assert "packages/agents/Claude.md" in _context(out), _context(out)[:500]


def test_docs_leaves_the_doc_a_read_prints_to_the_read():
    """`trace read <doc>` prints the doc itself, so the hook does not send it too."""
    doc = os.path.join(REPO, "packages", "agents", "Claude.md")
    rc, out, _ = _run(DOCS, {
        "tool_name": "Bash", "tool_input": {"command": f"trace read {doc}"},
        "cwd": REPO, "session_id": _sid("docs-read-doc"), "agent_id": "",
    })
    assert rc == 0
    assert "packages/agents/Claude.md" not in _context(out), _context(out)[:500]


def test_docs_writes_the_subagent_into_its_trace_calls():
    """A Subagent's shell names no agent, so its `trace` calls carry `--agent`
    and record into its own log, not the root agent's."""
    rc, out, _ = _run(DOCS, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace read {TARGET_FILE}", "run_in_background": False},
        "cwd": REPO, "session_id": _sid("docs-subagent"), "agent_id": "a1b2",
    })
    assert rc == 0
    rewritten = json.loads(out)["hookSpecificOutput"]["updatedInput"]
    assert rewritten == {"command": f"trace --agent a1b2 read {TARGET_FILE}", "run_in_background": False}


def test_reload_records_what_claude_loaded_so_it_is_not_sent_again():
    """InstructionsLoaded names a doc Claude Code loaded; tracer then skips it."""
    sid = _sid("reload-record")
    root_doc = os.path.join(REPO, "Claude.md")
    rc, _, _ = _run(RELOAD, {
        "hook_event_name": "InstructionsLoaded", "file_path": root_doc,
        "load_reason": "session_start", "cwd": REPO, "session_id": sid,
    })
    assert rc == 0
    rc, out, _ = _run(DOCS, {
        "tool_name": "Bash", "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": sid, "agent_id": "",
    })
    assert rc == 0
    ctx = _context(out)
    assert "packages/agents/Claude.md" in ctx and "## Claude.md\n" not in ctx, ctx[:500]


def test_reload_forgets_on_precompact():
    """Compaction drops what was loaded, so the record is forgotten before it."""
    sid = _sid("reload-compact")
    first = _run(DOCS, {
        "tool_name": "Bash", "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": sid, "agent_id": "",
    })
    assert _docs_surfaced(_context(first[1])) > 0
    rc, _, _ = _run(RELOAD, {"hook_event_name": "PreCompact", "cwd": REPO, "session_id": sid})
    assert rc == 0
    again = _run(DOCS, {
        "tool_name": "Bash", "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": sid, "agent_id": "",
    })
    assert _docs_surfaced(_context(again[1])) > 0, "docs must be sent again after compaction"


def test_reload_records_the_session_start_docs_after_compaction():
    sid = _sid("reload-after-compact")
    for event in ({"hook_event_name": "PreCompact"},
                  {"hook_event_name": "SessionStart", "source": "compact"}):
        rc, _, _ = _run(RELOAD, {**event, "cwd": REPO, "session_id": sid})
        assert rc == 0
    hooks_dir = os.path.join(REPO, "packages", "agents", "hooks")
    rc, out, _ = _run(DOCS, {
        "tool_name": "Bash", "tool_input": {"command": f"trace grep needle {hooks_dir}"},
        "cwd": REPO, "session_id": sid, "agent_id": "",
    })
    assert rc == 0
    ctx = _context(out)
    assert "## Claude.md\n" not in ctx and "collaboration.md" not in ctx, ctx[:500]


def test_reload_resends_the_user_rules_loaded_before_compaction(tmp_path, write_transcript):
    rule = tmp_path / "code.md"
    rule.write_text('---\npaths:\n  - "**/*.py"\n---\n\n### Write no comments\nGood Architecture documents itself.\n')
    path = write_transcript([{"type": "attachment", "attachment": {
        "type": "nested_memory", "path": str(rule),
        "content": {"path": str(rule), "type": "User", "content": "an older copy"}}}])
    rc, out, _ = _run(RELOAD, {"hook_event_name": "SessionStart", "source": "compact",
                               "transcript_path": path, "cwd": REPO,
                               "session_id": _sid("reload-user-rule")})
    assert rc == 0
    ctx = _context(out)
    assert "Contents of %s" % rule in ctx and "### Write no comments" in ctx
    assert "paths:" not in ctx and "an older copy" not in ctx


def test_reload_cuts_the_rule_that_does_not_fit_instead_of_naming_it(tmp_path, write_transcript):
    rules = []
    for name in ("first", "second"):
        rule = tmp_path / ("%s.md" % name)
        rule.write_text("### %s\n" % name + "".join("- %s rule %d\n" % (name, n) for n in range(300)))
        rules.append(rule)
    path = write_transcript([{"type": "attachment", "attachment": {
        "type": "nested_memory", "path": str(rule),
        "content": {"path": str(rule), "type": "User", "content": ""}}} for rule in rules])
    rc, out, _ = _run(RELOAD, {"hook_event_name": "SessionStart", "source": "compact",
                               "transcript_path": path, "cwd": REPO,
                               "session_id": _sid("reload-cut-rule")})
    assert rc == 0
    ctx = _context(out)
    assert len(json.loads(out)["hookSpecificOutput"]["additionalContext"]) <= 10_000
    assert "- first rule 299" in ctx and "- second rule 0" in ctx, ctx[-800:]
    assert "continue: trace read %s --lines " % rules[1] in ctx, ctx[-800:]


def test_rules_ignores_bash_trace_command():
    """inject_rules does NOT handle the Bash trace-command path — that's inject_docs'
    job. A `trace read` Bash event is a clean no-op."""
    rc, out, err = _run(RULES, {
        "tool_name": "Bash",
        "tool_input": {"command": f"trace read {TARGET_FILE}"},
        "cwd": REPO, "session_id": "rules-bash", "agent_id": "a",
    })
    assert rc == 0 and out == "" and err == ""
