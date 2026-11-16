"""The declarations an agent's definition file carries in its frontmatter.

Reading the block belongs to `lib.frontmatter`, which every definition file in
this repository is read through. This module locates the definition that
governs the running agent and reads one declaration off it.
"""

import os

from lib import frontmatter


def definition_path(name):
    """The definition file governing the running agent, or "" when none does.

    A name resolves the way Claude Code resolves it: the project's own
    .claude/agents/ first, then the *active* config root, because a profile
    carries its own agents/ directory and a name means whichever file that root
    holds.
    """
    if not name:
        return ""
    project = os.environ.get("CLAUDE_PROJECT_DIR")
    if project:
        local = os.path.join(project, ".claude", "agents", name + ".md")
        if os.path.isfile(local):
            return local
    root = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    return os.path.join(root, "agents", name + ".md")


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
    """
    try:
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    except (FileNotFoundError, NotADirectoryError):
        return None
    return frontmatter.declared(text, key)
