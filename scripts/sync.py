#!/usr/bin/env python3
"""Repo maintenance entry point.

Restows every package, regenerates the generated codex files, and rebuilds a
tracer plugin payload that has fallen behind its source. Run by setup.sh and the
pre-commit hook; idempotent.
"""

import os
import sys

import agents
import hooks
import stow
import tracer

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACKAGES = os.path.join(ROOT, "packages")

sys.path.insert(0, os.path.join(PACKAGES, "agents", "hooks"))
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
