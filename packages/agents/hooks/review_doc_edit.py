#!/usr/bin/env python3
"""Review a prompt-doc edit before it lands and flag low-quality docs (LLM).

Prompt markdown and hook prompt strings are gated. Shaping and plan artifacts
are left to the planning validators, so the doc reviewer cannot block ordinary
artifact edits. The hook assembles the full review context deterministically —
the post-edit file, its related docs, and the diff — and hands it to one
`run_model` call, exactly the inline-prompt gate shape the classifier and the
three validators already use (classify_intent.py, update_goal.py via
lib/model_call.py). The model returns block/polish findings; block findings are
raised as concerns, and polish findings warn.

Related docs are gathered by walking the file tree, not `trace docs`: the gate
needs every parent and child Claude.md WITH content on every call, while
`trace docs` dedups against the session log and omits content for already-loaded
ancestors.

Any infrastructure failure (recursion guard, model unavailable, parse error)
returns 0 and never blocks the edit — a broken model must not wedge doc work.
"""

import os
import sys

from lib import feedback
from lib.event import canonical_tool, field, read_event
from lib.model_call import run_model

BINDING = {
    "events": {"PreToolUse": ["Write", "Edit", "MultiEdit"]},
    "harness": "all",
    "timeout": 120,
}

SYSTEM_PROMPT = (
    "You are a documentation reviewer for an AI-agent codebase. Every markdown file "
    "here is read by AI agents, not end users: skills (SKILL.md + references), agent "
    "prompts, and Claude.md project docs. You review one pending edit to such a doc and "
    "return findings as JSON.\n"
    "\n"
    "You judge the POST-EDIT file (the result the reader will load), using the DIFF to "
    "see what changed and the RELATED files to check voice, precedent, and "
    "non-duplication. Judge only what the edit introduces or changes — never pre-existing "
    "text the edit doesn't touch.\n"
    "\n"
    "The TARGET may also be a Python hook file (.py); there the only reviewable content is "
    "its inline natural-language prompt and instruction strings — the text the hook sends "
    "to a model or shows an agent. Apply every category to that prose; the surrounding "
    "Python (code, comments, control flow) is not a doc and is never reviewed. If the diff "
    "touches no such prose, return empty arrays.\n"
    "\n"
    "Work deterministically: two reviews of the same file must return the same findings. "
    "Walk the file's sections from top to bottom. At each section, test it against the "
    "categories below in order. Report a finding ONLY when you can quote the exact "
    "offending text AND satisfy that category's evidence test; if you cannot, there is no "
    "finding — a vague sense a section 'could be tighter' is never a finding. Report at "
    "most one finding per (category, section), with location set to the section's exact "
    "heading. Hold one constant bar at every section: a section either meets a category's "
    "evidence test or it does not, regardless of how many findings you already have. "
    "Report EVERY section that meets the bar, in top-to-bottom order — never a "
    "representative subset, never stop early.\n"
    "\n"
    "Seven categories. Each states the evidence you must produce; no evidence, no finding:\n"
    "1. re-teaches-basics — explains a capability the AI reader already has, or states a "
    "requirement that corrects no default behavior. Evidence: quote the sentence AND name "
    "the exact default behavior it restates. Rule: cc 'Write Direct'.\n"
    "2. mechanism-before-result — a section opens with how something works instead of the "
    "result it produces or the rule it sets. Evidence: quote the opening line. Rule: "
    "claude-md 'front-load critical info'.\n"
    "3. voice-or-structure-mismatch — a heading or section breaks the file's OWN "
    "established pattern. Evidence: quote the offending heading AND quote a sibling that "
    "sets the pattern it breaks. Taste is not evidence. Rule: claude-md Style Guide.\n"
    "4. duplicate-or-misplaced — a section reproduces reference/parent detail instead of "
    "summarizing and pointing to it, or sits at the wrong hierarchy level. A one-line rule "
    "that links to a reference file is the skill doing its job and is NEVER this finding. "
    "Evidence: quote the duplicated detail here AND name the reference/parent file plus the "
    "guidance there it reproduces. No named other location, no finding. Rule: claude-md "
    "'never duplicate referenced content'.\n"
    "5. fluff — multiple sentences where one line carries the rule. Evidence: quote the "
    "passage. Rule: claude-md 'every line earns its place'.\n"
    "6. rule-doesnt-correct-a-default — an instruction the agent would follow unprompted. "
    "Evidence: quote it AND name the default it restates. Rule: cc '#1 litmus'.\n"
    "7. explains-known-mechanics — explains how a standard external thing works — a "
    "format, protocol, tool behavior, or basic computing fact any agent here already knows "
    "(how XML tags nest, what a CLI flag does). Redundant exposition between peers who both "
    "hold the fact. A step sequence or preferred procedure is NOT this — that is the "
    "author's ordering the reader cannot infer, and is what prompts are for. Evidence: "
    "quote it AND name the shared fact it restates. Rule: cc 'Context is finite'.\n"
    "\n"
    "Put a finding in 'block' when it makes the doc actively mislead or misfit: "
    "re-teaches-basics, voice-or-structure-mismatch, duplicate-or-misplaced, "
    "rule-doesnt-correct-a-default, explains-known-mechanics. Put fluff and "
    "mechanism-before-result in 'polish'. If "
    "the edit is good, return empty arrays. Each finding gives location (the exact "
    "heading), category, evidence (the quoted offending text plus the second quote its test "
    "requires), and rule. Do not invent findings to look thorough; do not "
    "omit a citable one. You only RAISE each concern; you never prescribe a fix, an edit, "
    "or a rewrite — the main agent has the full context and owns that."
)

JSON_SCHEMA = (
    '{"type":"object","properties":{'
    '"block":{"type":"array","items":{"type":"object","properties":{'
    '"category":{"type":"string"},"location":{"type":"string"},'
    '"evidence":{"type":"string"},"rule":{"type":"string"}},'
    '"required":["category","location","evidence"]}},'
    '"polish":{"type":"array","items":{"type":"object","properties":{'
    '"category":{"type":"string"},"location":{"type":"string"},'
    '"evidence":{"type":"string"},"rule":{"type":"string"}},'
    '"required":["category","location","evidence"]}}},'
    '"required":["block","polish"]}'
)


# --- deterministic context gather ----------------------------------------------

def _read(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except Exception:
        return None


def _post_edit_content(event, file_path):
    """The file as it will read after the edit applies."""
    tool = field(event, "tool_name", "")
    if tool == "Write":
        return field(event, "tool_input.content", "")
    current = _read(file_path) or ""
    if tool == "Edit":
        old = field(event, "tool_input.old_string", "")
        new = field(event, "tool_input.new_string", "")
        return current.replace(old, new, 1) if old else current
    if tool == "MultiEdit":
        edits = field(event, "tool_input.edits", []) or []
        for e in edits:
            old = e.get("old_string", "")
            new = e.get("new_string", "")
            if old:
                current = current.replace(old, new, 1)
        return current
    return current


def _diff_text(event):
    tool = field(event, "tool_name", "")
    if tool == "Write":
        return "(whole file written)\n+++\n" + field(event, "tool_input.content", "")
    if tool == "Edit":
        return ("--- old\n" + field(event, "tool_input.old_string", "")
                + "\n+++ new\n" + field(event, "tool_input.new_string", ""))
    if tool == "MultiEdit":
        out = []
        for e in field(event, "tool_input.edits", []) or []:
            out.append("--- old\n" + e.get("old_string", "")
                       + "\n+++ new\n" + e.get("new_string", ""))
        return "\n\n".join(out)
    return ""


def _repo_root(start_dir):
    d = start_dir
    while True:
        if os.path.isdir(os.path.join(d, ".git")):
            return d
        parent = os.path.dirname(d)
        if parent == d:
            return start_dir
        d = parent


_SKIP_DIRS = {"node_modules", ".git", ".tracer-cache", "build", "dist", "target", "vendor"}


def _inode(path):
    try:
        s = os.stat(path)
        return (s.st_dev, s.st_ino)
    except OSError:
        return None


def _claude_md_context(file_path, root):
    """Every parent and child Claude.md, each with content. Excludes the target.

    Identity is the inode, not the path string: on a case-insensitive filesystem
    `Claude.md` and `CLAUDE.md` are one file, so the target would otherwise be
    gathered as its own parent and read as a self-duplicate.
    """
    target_dir = os.path.dirname(os.path.abspath(file_path))
    target_key = _inode(file_path)
    out = []
    seen = {target_key}

    def add(role, p):
        key = _inode(p)
        if key is None or key in seen:
            return
        seen.add(key)
        out.append((role, p, _read(p)))

    # Parents: walk up from the target's directory to the repo root.
    d = target_dir
    while True:
        for name in ("Claude.md", "CLAUDE.md"):
            add("parent", os.path.join(d, name))
        if os.path.abspath(d) == os.path.abspath(root) or os.path.dirname(d) == d:
            break
        d = os.path.dirname(d)

    # Children: every Claude.md below the target's directory, skipping vendor trees.
    for dirpath, dirs, files in os.walk(target_dir):
        dirs[:] = [x for x in dirs if x not in _SKIP_DIRS]
        if os.path.abspath(dirpath) == target_dir:
            continue
        for name in files:
            if name in ("Claude.md", "CLAUDE.md"):
                add("child", os.path.join(dirpath, name))
    return out


def _skill_refs(file_path, content):
    """The reference files a SKILL.md links, each with content."""
    skill_dir = os.path.dirname(file_path)
    out = []
    seen = set()
    for token in content.split("("):
        ref = token.split(")")[0]
        if ref.endswith(".md") and "references/" in ref and ref not in seen:
            seen.add(ref)
            p = os.path.normpath(os.path.join(skill_dir, ref))
            body = _read(p)
            if body is not None:
                out.append(("reference", p, body))
    return out


def _related(file_path, content, root):
    base = os.path.basename(file_path)
    if base == "SKILL.md":
        return _skill_refs(file_path, content)
    if base in ("Claude.md", "CLAUDE.md"):
        return _claude_md_context(file_path, root)
    return []


def _build_prompt(file_path, content, diff, related, exists):
    rel_dir = os.path.relpath(file_path, _repo_root(os.path.dirname(file_path)))
    kind = "markdown doc" if file_path.endswith(".md") else "python hook file (review inline prompts only)"
    parts = [
        "TARGET: %s (%s, %s)" % (rel_dir, kind, "existing" if exists else "new file"),
        "",
        "POST-EDIT FILE:",
        content if content.strip() else "(empty)",
        "",
        "DIFF (what this edit changes):",
        diff if diff.strip() else "(none)",
    ]
    if related:
        parts += ["", "RELATED FILES (for voice, precedent, non-duplication):"]
        for role, path, body in related:
            parts += ["", "--- %s: %s ---" % (role, path), body or "(unreadable)"]
    return "\n".join(parts)


# --- emit ----------------------------------------------------------------------

def _format(findings):
    lines = []
    for f in findings:
        loc = f.get("location", "?")
        cat = f.get("category", "?")
        ev = f.get("evidence", "")
        rule = f.get("rule", "")
        lines.append("- [%s] %s%s%s"
                     % (cat, loc, ("\n  evidence: " + ev) if ev else "",
                        ("\n  rule: " + rule) if rule else ""))
    return "\n".join(lines)


def warn(msg):
    feedback.context("review_doc_edit", "PreToolUse", msg)


def block(msg):
    return feedback.raise_concern("review_doc_edit", "PreToolUse", msg)


def _planning_artifact(path):
    p = os.path.normpath(path)
    for directory in ("docs/shaping", "docs/plans", ".claude/shaping", ".claude/plans"):
        if (p == directory or p.startswith(directory + os.sep)
                or (os.sep + directory + os.sep) in p):
            return True
    return False


def _reviewable(path):
    """Prompt Markdown, plus the Python hook sources in agents/hooks — they carry
    the inline prompts agents read. Planning artifacts, lib/ helpers, and the
    tests tree are not hook sources. A hook's code-only edit yields a prose-free
    diff the reviewer returns empty on, so only prompt-touching edits are
    actually judged.
    """
    if not path:
        return False
    if _planning_artifact(path):
        return False
    if path.endswith(".md"):
        return True
    parent = os.path.dirname(path)
    return (path.endswith(".py")
            and os.path.basename(parent) == "hooks"
            and os.path.basename(os.path.dirname(parent)).endswith("agents"))


# --- main ----------------------------------------------------------------------

def main():
    # The model call runs a nested harness; never review our own review.
    if os.environ.get("CLAUDE_SESSION_HOOK") == "true":
        return 0

    event = read_event()
    if canonical_tool(event) != "write":
        return 0

    file_path = field(event, "tool_input.file_path", "")
    if not _reviewable(file_path):
        return 0

    exists = os.path.isfile(file_path)
    content = _post_edit_content(event, file_path)
    diff = _diff_text(event)
    root = _repo_root(os.path.dirname(os.path.abspath(file_path)))
    related = _related(file_path, content, root)

    prompt = _build_prompt(file_path, content, diff, related, exists)
    result = run_model("high", system_prompt=SYSTEM_PROMPT, user_prompt=prompt,
                       schema=JSON_SCHEMA, session_id=field(event, "session_id", ""),
                       hook="review_doc_edit")
    if not result:
        return 0

    blocks = result.get("block") or []
    polish = result.get("polish") or []

    if blocks:
        report = "Potential issues with this doc edit:\n\n" + _format(blocks)
        if polish:
            report += "\n\nPolish (not blocking):\n" + _format(polish)
        return block(report)

    if polish:
        warn("Doc polish (not blocking):\n" + _format(polish))
    return 0


if __name__ == "__main__":
    sys.exit(main())
