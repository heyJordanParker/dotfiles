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

# Entries stow must not lay down, per package. The claude package's `skills`,
# `commands`, and `agents` are symlinks into the shared roster; folding them makes
# ~/.claude/<name> resolve into this repo, so anything a third-party tool installs
# there lands in the tracked tree and its absolute symlink then aborts every later
# stow run. restow points those targets at the real ~/.agents/<name> instead.
SHARED = ("skills", "commands", "agents")
IGNORE = {"claude": r"^(%s)$" % "|".join(SHARED)}


def _relink(link, target):
    """Point `link` at `target`, replacing an existing symlink that points elsewhere."""
    if os.path.islink(link):
        if os.path.realpath(link) == os.path.realpath(target):
            return
        os.unlink(link)
    elif os.path.exists(link):
        raise RuntimeError("%s exists and is not a symlink; move it aside" % link)
    os.symlink(target, link)


def restow(packages_dir):
    plan = dict(TARGETS)
    for pkg in CONFIG:
        plan[pkg] = f"{HOME}/.config/{pkg}"
    # Each shared roster target must be a real dir so stow links its contents
    # per-child, letting foreign tools' installs coexist there instead of folding
    # the whole dir into one symlink.
    for name in SHARED:
        target = f"{HOME}/.agents/{name}"
        # A folded symlink here would make makedirs a no-op and let stow re-fold,
        # so drop it and let the next stow pass link the roster per-child.
        if os.path.islink(target):
            os.unlink(target)
        os.makedirs(target, exist_ok=True)
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
        cmd = ["stow", "-R", "-t", target, pkg]
        if pkg in IGNORE:
            cmd.append("--ignore=" + IGNORE[pkg])
        subprocess.run(cmd, cwd=packages_dir, check=True)
    # Both names now reach one real directory per roster, so anything a
    # third-party tool installs lands outside the repo whichever path it writes.
    for name in SHARED:
        _relink(f"{HOME}/.claude/{name}", f"{HOME}/.agents/{name}")
