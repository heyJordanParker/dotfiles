"""Generate codex artifacts from the shared agent definitions.

Reads packages/agents/agents/*.md and writes two siblings codex auto-discovers:
<name>.toml (the subagent definition) and <name>.prompt.md (the frontmatter-
stripped body, used as a base-instructions override via model_instructions_file).
Frontmatter name/description map across; the body becomes developer_instructions.
model/tools/skills/color/memory are dropped — codex has no key for them and our
model names aren't codex models. Both artifacts are gitignored; this regenerates them.
"""

import glob
import os

import files
import frontmatter


def generate(agents_dir):
    written = []
    for md in sorted(glob.glob(os.path.join(agents_dir, "*.md"))):
        if md.endswith(".prompt.md"):
            continue
        fields, body = frontmatter.parse(_read(md))
        name = fields.get("name") or os.path.splitext(os.path.basename(md))[0]
        out = os.path.splitext(md)[0] + ".toml"
        _write(out, _render(name, fields.get("description", ""), body))
        written.append(out)
        prompt = os.path.splitext(md)[0] + ".prompt.md"
        _write(prompt, body.strip() + "\n")
        written.append(prompt)
    return written


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
