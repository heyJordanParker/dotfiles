#!/usr/bin/env python3
"""Enforce the Prompt Architecture on every prompt-doc edit (LLM).

Prompt markdown and hook prompt strings are gated. Shaping and plan artifacts
are left to the planning validators, so the doc reviewer cannot block ordinary
artifact edits. The hook assembles the full review context deterministically —
the post-edit file, its related docs, the diff, and the law itself
(docs/architecture/Architecture.md, Domain.md, the /cc SKILL.md) — and hands it
to one `run_model` call. Structural findings (a block outside its allowance,
its home, its shape, its words, its reader's need, or its one job) block the
edit with exit 2 so the agent fixes and retries in the same turn; the one
advisory finding (no-disposition-change) is raised as a concern.

Related docs are gathered by walking the file tree, not `trace docs`: the gate
needs every parent and child Claude.md WITH content on every call, while
`trace docs` dedups against the session log and omits content for already-loaded
ancestors.

Any infrastructure failure (recursion guard, model unavailable, parse error)
returns 0 and never blocks the edit — a broken model must not wedge doc work.
"""

import os
import re
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
    "You enforce the Prompt Architecture of an AI-agent codebase. Every markdown file "
    "here is read by AI agents, not end users: skills (SKILL.md + references), agent "
    "prompts, rules files, and Claude.md project docs. You review one pending edit to "
    "such a doc against THE LAW files provided (the Prompt Architecture with its "
    "per-file allowances and one-home-per-block table, Domain.md with the domain's "
    "words, and the /cc skill with the block shapes) and return findings as JSON.\n"
    "\n"
    "You judge the POST-EDIT file (the result the reader will load), using the DIFF to "
    "see what changed and the RELATED files to check homes and duplication. Judge only "
    "what the edit introduces or changes — never pre-existing text the edit doesn't "
    "touch.\n"
    "\n"
    "The TARGET may also be a Python hook file (.py); there the only reviewable content is "
    "its inline natural-language prompt and instruction strings — the text the hook sends "
    "to a model or shows an agent. Apply every category to that prose; the surrounding "
    "Python (code, comments, control flow) is not a doc and is never reviewed. If the diff "
    "touches no such prose, return empty arrays.\n"
    "\n"
    "Work deterministically: two reviews of the same file must return the same findings. "
    "A finding you would not report on every rerun is borderline — do not report it; "
    "block findings stop real work, so only certain violations block. "
    "Report a finding ONLY when you can quote the exact offending text AND satisfy that "
    "category's evidence test; if you cannot, there is no finding — a vague sense a "
    "section 'could be tighter' is never a finding. Report at most one finding per "
    "(category, section), with location set to the section's exact heading. Report EVERY "
    "section that meets a bar, in top-to-bottom order — never a representative subset, "
    "never stop early.\n"
    "\n"
    "Six STRUCTURAL categories — these go in 'block'. Each states the evidence you must "
    "produce; no evidence, no finding:\n"
    "1. wrong-home — a building block sits in a file whose allowance excludes it: a "
    "Process in an agent file, Rules or vocabulary in a Claude.md, a Frame in a SKILL.md, "
    "a Decision inline, a roster or catalog split into a reference. An agent file's "
    "Principles are allowed there; whether a line in a Principles section is instead a "
    "Rule is decided by Domain.md's Principle and Rule definitions, and it is a finding "
    "only when it is unmistakably a Rule by those definitions — ambiguous means "
    "Principle, no finding. Evidence: quote the block, name the file type, and quote the "
    "Architecture allowance line it violates.\n"
    "2. copied-not-named — a block restates what another home owns instead of naming that "
    "home, per the one-home-per-block table. A one-line pointer to the home is the system "
    "working and is NEVER this finding. A hook's inline message that states what that "
    "hook's own code enforces mirrors the code beside it, not another home — never this "
    "finding. Evidence: quote the copy here AND name the home file plus the text there it "
    "reproduces. No named home, no finding.\n"
    "3. wrong-shape — a block is not written in its /cc shape: a Rule without a '###' "
    "action title, a Condition as anything but an 'IF ...:' line directly above the one "
    "rule it owns, a Fact outside a plain listicle sentence, an 'Example:'/'Never:' line "
    "not labeled directly under its rule, or a trigger section in a body when the "
    "frontmatter description is the only trigger. Evidence: quote the malformed block AND "
    "name the shape it should have.\n"
    "4. coined-term — an invented capitalized term of art: a name for a concept this "
    "system made up, tracing to neither Domain.md nor the code. Universal technical "
    "vocabulary (CMS, API, CI, REST, frontmatter, markdown) and plain-English "
    "descriptions are NEVER coined — only names that read as this project's own "
    "vocabulary without being in Domain.md. A name that traces to the code counts as "
    "defined: a harness tool (Monitor, SendMessage), a file, a CLI, a frontmatter key. "
    "Evidence: quote the term AND state that no provided Domain.md defines it "
    "and no tool, file, or identifier carries the name.\n"
    "5. over-context — a passage the file's reader does not consume to do the file's "
    "job: a caveat about a sibling agent's work, a boundary the reader cannot cross "
    "anyway, background restating a settled decision, or narration of how the file came "
    "to be. A single clause that grounds a rule the reader must follow is NOT this — "
    "only passages whose removal changes nothing the reader does. Evidence: quote the "
    "passage AND name the reader's job it plays no part in.\n"
    "6. bundled-job — a skill's Process, an agent's frame, or a command owns two "
    "unrelated jobs; two observed gaps means two prompts. This NEVER applies to a rules "
    "file or a Claude.md — those are collections of Rules and Facts by design, and many "
    "unrelated entries is their normal state. Evidence: name both jobs and quote a line "
    "of each.\n"
    "\n"
    "One ADVISORY category — this goes in 'polish':\n"
    "7. no-disposition-change — the prompt would not change the agent's behavior: it "
    "states what the agent already knows or would already do unprompted. Evidence: quote "
    "the passage AND name the default behavior or known fact it restates.\n"
    "\n"
    "If the edit is good, return empty arrays. Each finding gives location (the exact "
    "heading), category, evidence (the quoted offending text plus the second quote or "
    "name its test requires), and rule (the law line it enforces). Do not invent findings "
    "to look thorough; do not omit a citable one. You only RAISE each finding; you never "
    "prescribe a fix, an edit, or a rewrite — the main agent has the full context and "
    "owns that."
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


def _patch_text(event):
    return (field(event, "tool_input.input", "") or field(event, "tool_input.patch", "")
            or field(event, "tool_input.changes", ""))


def _patch_file_path(event):
    """The file a codex apply_patch touches; '' when the event is not a patch."""
    m = re.search(r"^\*\*\* (?:Update|Add) File: (.+)$", _patch_text(event), re.M)
    return m.group(1).strip() if m else ""


def _post_edit_content(event, file_path):
    """The file as it will read after the edit applies."""
    tool = field(event, "tool_name", "")
    if tool == "Write":
        return field(event, "tool_input.content", "")
    if tool == "apply_patch":
        current = _read(file_path) or ""
        added = "\n".join(ln[1:] for ln in _patch_text(event).splitlines()
                          if ln.startswith("+") and not ln.startswith("+++"))
        # A patch's post-image cannot be rebuilt without a full patch engine;
        # judge the current file plus every added line, which carries what the
        # edit introduces — the only text the reviewer may fault anyway.
        return (current + "\n" + added) if current else added
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
    if tool == "apply_patch":
        return _patch_text(event)
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


_SKIP_DIRS = {"node_modules", ".git", ".tracer-cache", "build", "dist", "target", "vendor",
              "worktrees"}


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
    """Every reference beside a SKILL.md, linked or not.

    The whole directory, not just linked files: a SKILL.md that copies a
    reference's process while dropping the pointer would otherwise hide the
    home from the reviewer — the exact copied-not-named case.
    """
    ref_dir = os.path.join(os.path.dirname(file_path), "references")
    out = []
    try:
        names = sorted(os.listdir(ref_dir))
    except OSError:
        return out
    for name in names:
        if name.endswith(".md"):
            p = os.path.join(ref_dir, name)
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


_CC_DIR = os.path.expanduser("~/.claude/skills/cc")


def _law(root):
    """The files the gate judges against, freshest copy on every call.

    The Architecture, the Domain and the /cc Process are one global law, read
    where /cc itself reads them — beside its SKILL.md. The edited project's own
    Domain.md joins them when it has one, because coined-term judges a term
    against the vocabulary that project defines. Identity is the inode, so in
    this repository the symlink and its target count once.
    """
    paths = [
        ("law:architecture", os.path.join(_CC_DIR, "Architecture.md")),
        ("law:domain", os.path.join(_CC_DIR, "Domain.md")),
        ("law:cc", os.path.join(_CC_DIR, "SKILL.md")),
        ("law:domain:project", os.path.join(root, "Domain.md")),
    ]
    out = []
    seen = set()
    for role, path in paths:
        key = _inode(path)
        if key is None or key in seen:
            continue
        body = _read(path)
        if body is None:
            continue
        seen.add(key)
        out.append((role, path, body))
    return out


def _build_prompt(file_path, content, diff, related, law, exists):
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
    if law:
        parts += ["", "THE LAW (judge against these):"]
        for role, path, body in law:
            parts += ["", "--- %s: %s ---" % (role, path), body]
    if related:
        parts += ["", "RELATED FILES (for homes and non-duplication):"]
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
    return feedback.block("review_doc_edit", msg)


_PROMPT_BASENAMES = {"Claude.md", "CLAUDE.md", "Domain.md", "SKILL.md"}
_PROMPT_DIRS = {"agents", "commands", "rules", "references"}


def _reviewable(path):
    """Only Prompts are reviewed, by allowlist: the named Prompt files, .md files
    whose directory is a Prompt home, and the Python hook sources in agents/hooks
    (they carry the inline prompts agents read; a code-only edit yields a
    prose-free diff the reviewer returns empty on). Everything else — plans,
    shaping, Evidence, READMEs, scratch files — is an artifact, not a Prompt,
    and never reviewed. Artifact stores like docs/agents/<slug>/plan.md pass
    because their parent is the slug, not a Prompt home.
    """
    if not path:
        return False
    parent = os.path.basename(os.path.dirname(path))
    if path.endswith(".py"):
        return (parent == "hooks"
                and os.path.basename(os.path.dirname(os.path.dirname(path))).endswith("agents"))
    if not path.endswith(".md"):
        return False
    if os.path.basename(path) in _PROMPT_BASENAMES:
        return True
    if parent in _PROMPT_DIRS:
        # docs/agents is the Evidence store, not the Agent roster.
        return "docs" + os.sep + "agents" not in os.path.normpath(path)
    # A skill's own SKILL.md siblings (skills/<name>/*.md) are Prompts too.
    return os.path.basename(os.path.dirname(os.path.dirname(path))) == "skills"


# --- main ----------------------------------------------------------------------

def main():
    # The model call runs a nested harness; never review our own review.
    if os.environ.get("CLAUDE_SESSION_HOOK") == "true":
        return 0

    event = read_event()
    if canonical_tool(event) != "write":
        return 0

    file_path = field(event, "tool_input.file_path", "") or _patch_file_path(event)
    if not _reviewable(file_path):
        return 0

    exists = os.path.isfile(file_path)
    content = _post_edit_content(event, file_path)
    diff = _diff_text(event)
    root = _repo_root(os.path.dirname(os.path.abspath(file_path)))
    related = _related(file_path, content, root)

    prompt = _build_prompt(file_path, content, diff, related, _law(root), exists)
    result = run_model("medium", system_prompt=SYSTEM_PROMPT, user_prompt=prompt,
                       schema=JSON_SCHEMA)
    if not result:
        # Fail open, but never silently: the agent and the architect see that
        # this edit landed unreviewed instead of mistaking the outage for a pass.
        warn("model unavailable — this prompt-doc edit landed UNREVIEWED")
        return 0

    blocks = result.get("block") or []
    polish = result.get("polish") or []

    if blocks:
        report = "BLOCKED: this edit breaks the Prompt Architecture. Fix and retry.\n\n" + _format(blocks)
        if polish:
            report += "\n\nAdvisory (not blocking):\n" + _format(polish)
        return block(report)

    if polish:
        warn("Advisory (not blocking):\n" + _format(polish))
    return 0


if __name__ == "__main__":
    sys.exit(main())
