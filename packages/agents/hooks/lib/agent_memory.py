"""The declarations an agent's definition file carries in its frontmatter.

One definition file is the whole agent on both harnesses, so a declaration in it
means the same thing on both. Every declaration is read here: the restrictions
(`memory: none`, `readonly: true`), the one grant (`ssh: enabled`), and the
settings a run resolves (`codex-model`, `effort`, `harness`). They all come off
the same line shape in the same file from the same roster, and defining that
parse a second time is how the memory syntax nearly drifted in the first place.

The gates did not share code once, and the syntax was defined twice — the Claude
gate and the codex gate could silently drift apart. This module is the single
definition; each gate keeps only its own way of locating the file, which
`definition_path` answers for both.

The failure directions are not symmetric, and each declaration names its own. A
*missing* restriction must leave the capability reachable: that is the contract
every agent without the key relies on. An *explicit* restriction must never
quietly become permission, so a definition that exists and cannot be read denies.
A *grant* inverts only the last part — unreadable withholds it, because a grant
has to be read to be given.

Reading the block belongs to `lib.frontmatter`, which every definition file in
this repository is read through. What stays here is what only this module knows:
which key means a restriction, which means a grant, and what an unreadable
definition answers for each.
"""

import os

from lib import frontmatter

# The environment variable carrying the running agent's definition path into a
# codex run. Named here because both ends read declarations through this module:
# codex_run.py sets it when it launches, and the codex-side gates read it to find
# the file whose declarations they enforce. A codex session that is not a
# codex-run agent does not set it, and an absent value means no agent to gate.
AGENT_FILE_VAR = "CODEX_RUN_AGENT_FILE"

# The keys whose answer depends on what an unreadable definition means. Each has
# its own reader below; `declaration` refuses them so no caller can reach one
# through the reader that treats unreadable as undeclared.
_PERMISSION = ("memory", "readonly", "ssh")


def definition_path(name):
    """The definition file governing the running agent, or "" when none does.

    A codex run carries its own path, exported by its launcher, which is the
    identity that run was founded on. On Claude the name resolves under the
    *active* config root, because a profile carries its own agents/ directory and
    a name means whichever file that root holds.
    """
    exported = os.environ.get(AGENT_FILE_VAR, "")
    if exported:
        return exported
    if not name:
        return ""
    root = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    return os.path.join(root, "agents", name + ".md")


def denies_memory(path):
    """Whether the definition at `path` denies its agent Memory.

    True for an explicit `memory: none`, and for a definition that exists but
    cannot be read — an unreadable declaration is not evidence of permission.
    False when the file is absent or carries no `memory: none`, the undeclared
    case the contract leaves Memory reachable for.
    """
    return _declares(path, "memory", "none", unreadable=True)


def is_readonly(path):
    """Whether the definition at `path` declares `readonly: true`.

    True for a definition that exists and cannot be read, the same direction as
    `memory`: a restriction that was written must never lapse into permission
    because a read failed. Absent key and absent file both mean the agent writes,
    which is what every agent without the key relies on.
    """
    return _declares(path, "readonly", "true", unreadable=True)


def allows_ssh(path):
    """Whether the definition at `path` declares `ssh: enabled`.

    The one opt-in declaration: absence denies. An unreadable definition denies
    too, because the grant has to be read to be given, and the agent that loses a
    reachable machine says so in its report — where a silent grant to an agent
    that never declared one says nothing.
    """
    return _declares(path, "ssh", "enabled", unreadable=False)


def _declares(path, key, value, unreadable):
    """Whether `key` carries `value`, with `unreadable` as the answer for a
    definition that exists and cannot be read.

    Lowering the declared value belongs here rather than in the parse: every
    caller of this reads one fixed word, and spelling it `True` or `None` is the
    same declaration. `declaration` returns values whose meaning it does not know
    and never lowers them.
    """
    try:
        text = _read(path)
    except (OSError, UnicodeDecodeError):
        return unreadable
    if text is None:
        return False
    declared = frontmatter.declared(text, key)
    return declared is not None and declared.lower() == value


def declaration(path, key):
    """The frontmatter value the definition at `path` declares for `key`.

    None when the file is absent or carries no such key — both are the undeclared
    case, and the caller supplies its own default. The value comes back exactly
    as written, because this function does not know what its caller's values
    mean.

    A definition that exists but cannot be read raises rather than reading as
    undeclared: a declaration that was written and did not take effect is a
    broken agent, and silently running the default would hide it behind an
    output line that looks exactly like a correct run.

    Never read a permission key through here. Their contract turns on what an
    unreadable definition means, and this returns None for one — the permissive
    answer for every one of them. `denies_memory`, `is_readonly` and `allows_ssh`
    are the only correct readers of those keys, so asking for one here is a
    mistake rather than a request.
    """
    if key in _PERMISSION:
        raise ValueError(
            "read the %s declaration through its own reader, which decides what "
            "an unreadable definition means" % key)
    text = _read(path)
    return None if text is None else frontmatter.declared(text, key)


def _read(path):
    """The file's text, or None when it is absent. Other read failures raise."""
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except (FileNotFoundError, NotADirectoryError):
        return None


