#!/usr/bin/env python3
"""Repo maintenance entry point.

Restows every package and regenerates the Hook wiring. Run by setup.sh and the
pre-commit hook; idempotent.
"""

import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACKAGES = os.path.join(ROOT, "packages")

sys.path.insert(0, os.path.join(PACKAGES, "agents", "hooks"))

import hooks  # noqa: E402
import stow  # noqa: E402
from lib.session_state import clear_stale_locks, migrate_sessions  # noqa: E402


def main():
    stow.restow(PACKAGES)
    migrate_sessions()
    clear_stale_locks()
    hooks.generate(
        os.path.join(PACKAGES, "agents", "hooks"),
        os.path.join(PACKAGES, "claude", "settings.json"),
        os.path.join(PACKAGES, "codex-system", "config.toml"),
        os.path.join(PACKAGES, "claude", "profiles"),
    )


if __name__ == "__main__":
    main()
