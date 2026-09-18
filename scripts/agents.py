"""Generate codex artifacts from the shared agent definitions.

Reads packages/agents/agents/*.md and writes <name>.prompt.md beside each
definition: the frontmatter-stripped body with its named skills inlined, sent
inline as a run's baseInstructions by codex-run and pointed at by config.toml's
model_instructions_file for the interactive session. model/tools/color are
dropped — codex has no key for them, and `model` names a Claude model.

No `<name>.toml` role artifact is written. codex spawns a sub-agent under an
`agent_type`, and a role backed by a `config_file` — declared in config.toml or
discovered in ~/.codex/agents — fails to apply in codex 0.153.4: the spawn
answers `agent type is currently not available`, while a built-in role with no
config file spawns. A role file would therefore govern nothing, and its presence
shadowed codex's own `explorer` role. What governs a codex sub-agent instead is
the role name codex puts on every hook payload, which `lib/agent_memory.py`
resolves to that agent's definition.

The artifact is gitignored; this regenerates it.
"""

import glob
import os
import sys

# The frontmatter reader lives with the hooks, so one definition of a declaration
# serves this generator and the gates that run from ~/.agents. Inserted before the
# import because a module-level import resolves at import time, and this module
# runs standalone as well as through sync.py.
sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "packages", "agents", "hooks"))

import files  # noqa: E402
from lib import frontmatter  # noqa: E402


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
        prompt = os.path.splitext(md)[0] + ".prompt.md"
        _write(prompt, body.strip() + "\n")
        written.append(prompt)
    return written


def generate_profiles(profiles_dir):
    """Generate the same artifact for each profile's own agents.

    A profile is its own config root with its own roster, and `codex-run`
    resolves against the active root, so a profile agent needs the artifact a
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


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def _write(path, text):
    files.write_if_changed(path, text)


if __name__ == "__main__":
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    packages = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "packages")
    written = generate(os.path.join(packages, "agents", "agents"))
    written += generate_profiles(os.path.join(packages, "claude", "profiles"))
    print(f"{len(written)} artifacts written")
