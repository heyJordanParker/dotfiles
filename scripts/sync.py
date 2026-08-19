#!/usr/bin/env python3
"""Repo maintenance entry point.

Restows every package, regenerates the generated codex files, and rebuilds a
tracer plugin payload that has fallen behind its source. Run by setup.sh and the
pre-commit hook; idempotent.
"""

import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACKAGES = os.path.join(ROOT, "packages")

# Before the local imports: `agents` reads definition frontmatter through
# `lib.frontmatter`, which lives with the hooks so both the generator here and the
# gates running from ~/.agents read one definition of a declaration.
sys.path.insert(0, os.path.join(PACKAGES, "agents", "hooks"))

import agents  # noqa: E402
import hooks  # noqa: E402
import stow  # noqa: E402
import tracer  # noqa: E402
from lib.session_state import migrate_sessions  # noqa: E402


def main():
    stow.restow(PACKAGES)
    migrate_sessions()
    agents.generate(os.path.join(PACKAGES, "agents", "agents"))
    agents.generate_profiles(os.path.join(PACKAGES, "claude", "profiles"))
    hooks.generate(
        os.path.join(PACKAGES, "agents", "hooks"),
        os.path.join(PACKAGES, "claude", "settings.json"),
        os.path.join(PACKAGES, "codex", "config.toml"),
        os.path.join(PACKAGES, "claude", "profiles"),
    )
    tracer.sync(ROOT)


if __name__ == "__main__":
    main()
