"""Stow package -> target mapping and restow.

Single source of the package -> target mapping (moved here from setup.sh). Both
setup.sh and the pre-commit hook restow through this.
"""

import os
import subprocess

HOME = os.path.expanduser("~")

# package -> target directory
TARGETS = {
    "git": HOME,
    "hyprspace": HOME,
    "tmux": HOME,
    "zsh": HOME,
    "claude": f"{HOME}/.claude",
    "codex": f"{HOME}/.codex",
    "agents": f"{HOME}/.agents",
    "ssh": f"{HOME}/.ssh",
    "bin": f"{HOME}/.local/bin",
    "starship": f"{HOME}/.config",
}

# packages that each land in their own ~/.config/<pkg> directory
CONFIG = [
    "atuin", "bat", "borders", "btop", "bun", "delta", "ghostty", "hunk",
    "karabiner", "lazygit", "nvim", "opencode", "superfile", "zed", "zellij",
]


def restow(packages_dir):
    plan = dict(TARGETS)
    for pkg in CONFIG:
        plan[pkg] = f"{HOME}/.config/{pkg}"
    # ~/.agents/skills must be a real dir so stow links skills per-child, letting
    # foreign tools' skills coexist there instead of folding the whole dir.
    os.makedirs(f"{HOME}/.agents/skills", exist_ok=True)
    # Each profile under packages/claude/profiles/ is its own config root. Its
    # target must be a real dir so the claude stow pass links its config files
    # per-child instead of folding it into one symlink — that keeps Claude's
    # runtime/credential JSON, written into the target, out of the tracked repo.
    profiles = os.path.join(packages_dir, "claude", "profiles")
    if os.path.isdir(profiles):
        for name in os.listdir(profiles):
            if os.path.isdir(os.path.join(profiles, name)):
                os.makedirs(f"{HOME}/.claude/profiles/{name}", exist_ok=True)
    for pkg, target in plan.items():
        os.makedirs(target, exist_ok=True)
        subprocess.run(["stow", "-R", "-t", target, pkg], cwd=packages_dir, check=True)
