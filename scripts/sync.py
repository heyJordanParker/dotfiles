#!/usr/bin/env python3
"""Repo maintenance entry point.

Restows every package and regenerates the generated codex files. Run by setup.sh
and the pre-commit hook; idempotent.
"""

import os

import agents
import hooks
import stow

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACKAGES = os.path.join(ROOT, "packages")


def main():
    stow.restow(PACKAGES)
    agents.generate(os.path.join(PACKAGES, "agents", "agents"))
    hooks.generate(
        os.path.join(PACKAGES, "agents", "hooks"),
        os.path.join(PACKAGES, "claude", "settings.json"),
        os.path.join(PACKAGES, "codex", "config.toml"),
    )


if __name__ == "__main__":
    main()
