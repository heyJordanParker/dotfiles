"""Generate codex artifacts from the shared agent definitions.

Reads packages/agents/agents/*.md and writes two siblings codex auto-discovers:
<name>.toml (the subagent definition) and <name>.prompt.md (the frontmatter-
stripped body, used as a base-instructions override via model_instructions_file).
Frontmatter name/description map across; named skills are inlined into the body,
which becomes developer_instructions. model/tools/color are dropped — codex has no
key for them, and `model` names a Claude model. memory, codex-model, and effort are
dropped here too but are not lost: codex-run reads them straight off the definition
file at run time, so they reach a resumed run as well as a founding one. Both artifacts are
gitignored; this regenerates them.

The .prompt.md opens with an HTML-comment marker naming the agent it belongs to.
codex records the base instructions verbatim in the thread's rollout, so the
marker rides into the record and `codex-run resume` reads the founding agent's
name straight out of it instead of matching the prose back against the corpus.
The comment renders as nothing and instructs nothing — it carries identity only.
"""

import glob
import os

import files
import frontmatter


def generate(agents_dir):
    skills_dir = os.path.join(os.path.dirname(agents_dir), "skills")
    written = []
    for md in sorted(glob.glob(os.path.join(agents_dir, "*.md"))):
        if md.endswith(".prompt.md") or os.path.islink(md):
            # A symlinked definition is another roster's agent borrowed by name;
            # it generates where it really lives, and generating it again here
            # would put a second copy of the same artifact in this directory.
            continue
        fields, body = frontmatter.parse(_read(md))
        name = fields.get("name") or os.path.splitext(os.path.basename(md))[0]
        body = _compose(body, _load_skills(name, fields.get("skills"), skills_dir))
        out = os.path.splitext(md)[0] + ".toml"
        _write(out, _render(name, fields.get("description", ""), body))
        written.append(out)
        prompt = os.path.splitext(md)[0] + ".prompt.md"
        _write(prompt, _marker(name) + body.strip() + "\n")
        written.append(prompt)
    return written


def generate_profiles(profiles_dir):
    """Generate the same artifacts for each profile's own agents.

    A profile is its own config root with its own roster, and `codex-run`
    resolves against the active root, so a profile agent needs the artifacts a
    shared one has or it is Claude-only. A symlinked agents/ is the shared roster
    under another name and is skipped — it generates where it really lives.
    """
    written = []
    if not os.path.isdir(profiles_dir):
        return written
    for name in sorted(os.listdir(profiles_dir)):
        agents_dir = os.path.join(profiles_dir, name, "agents")
        if os.path.isdir(agents_dir) and not os.path.islink(agents_dir):
            written.extend(generate(agents_dir))
    return written


def _marker(name):
    # Read back by lib/codex_run.py's _marked_agent on resume; the two spellings
    # are pinned together by tests/hooks/test_codex_run.py.
    return "<!-- codex-run agent: %s -->\n\n" % name


def _load_skills(agent, value, skills_dir):
    skills = []
    for skill in _skill_names(value):
        path = os.path.join(skills_dir, skill, "SKILL.md")
        if not os.path.exists(path):
            raise FileNotFoundError(
                f"agent {agent!r} names missing skill {skill!r}: {path}"
            )
        fields, body = frontmatter.parse(_read(path))
        # disable-model-invocation skills are non-preloadable in Claude
        # (writing-agents.md); skipping them here keeps codex identical.
        if str(fields.get("disable-model-invocation", "")).lower() == "true":
            continue
        skills.append((skill, body))
    return skills


def _skill_names(value):
    if not value:
        return []
    value = value.strip()
    if value.startswith("[") and value.endswith("]"):
        value = value[1:-1]
    return [_unquote_name(item.strip()) for item in value.split(",") if item.strip()]


def _unquote_name(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
        return value[1:-1].strip()
    return value


def _compose(body, skills):
    sections = [body.strip()]
    if skills:
        sections.append("## Skills")
        for name, skill_body in skills:
            sections.append(f"### /{name}\n\n{skill_body.strip()}")
    return "\n\n".join(section for section in sections if section).strip()


def _render(name, description, body):
    return (
        f"name = {_basic(name)}\n"
        f"description = {_basic(description)}\n\n"
        f"developer_instructions = {_multiline(body)}\n"
    )


def _basic(s):
    s = s.replace("\\", "\\\\").replace('"', '\\"')
    return '"' + s.replace("\n", "\\n").replace("\t", "\\t") + '"'


def _multiline(body):
    # Literal multiline preserves the body verbatim (markdown is full of
    # backslashes); only escape into a basic block if the body itself holds the
    # literal delimiter.
    body = body.rstrip("\n")
    if "'''" not in body:
        return f"'''\n{body}\n'''"
    esc = body.replace("\\", "\\\\").replace('"""', '\\"\\"\\"')
    return f'"""\n{esc}\n"""'


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def _write(path, text):
    files.write_if_changed(path, text)


if __name__ == "__main__":
    import sys
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    packages = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "packages")
    written = generate(os.path.join(packages, "agents", "agents"))
    written += generate_profiles(os.path.join(packages, "claude", "profiles"))
    print(f"{len(written)} artifacts written")
