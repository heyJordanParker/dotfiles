"""External binary dependency checking.

Every CLI command except `doctor` calls `require_dependencies()` before doing
work. If a binary is missing the command exits 1 with per-platform install
instructions. `doctor` calls `check_dependencies()` directly so it can report
status rather than abort.
"""

from __future__ import annotations

import platform
import shutil
import sys

import click

REQUIRED_BINARIES: dict[str, dict[str, str]] = {
    "ast-grep": {
        "darwin": "brew install ast-grep",
        "linux": "https://ast-grep.github.io/guide/quick-start.html",
        "windows": "scoop install ast-grep",
    },
    "scc": {
        "darwin": "brew install scc",
        "linux": "https://github.com/boyter/scc#installation",
        "windows": "scoop install scc",
    },
    "ctags": {
        "darwin": "brew install universal-ctags",
        "linux": "apt install universal-ctags  # or pacman -S ctags",
        "windows": "https://github.com/universal-ctags/ctags",
    },
    "git": {
        "darwin": "xcode-select --install",
        "linux": "apt install git  # or your package manager",
        "windows": "https://git-scm.com/download/win",
    },
    "rg": {
        "darwin": "brew install ripgrep",
        "linux": "apt install ripgrep  # or your package manager",
        "windows": "scoop install ripgrep",
    },
}


def detect_platform() -> str:
    system = platform.system().lower()
    if system in {"darwin", "linux", "windows"}:
        return system
    return "linux"


def _is_universal_ctags() -> bool:
    """Distinguish universal-ctags (required) from BSD ctags (won't work).

    Universal-ctags supports JSON output and the rich field set the
    `structure` command relies on; BSD ctags does not.
    """
    import subprocess
    try:
        result = subprocess.run(
            ["ctags", "--version"], capture_output=True, text=True, timeout=5
        )
        return "Universal Ctags" in result.stdout
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def check_dependencies() -> tuple[list[str], list[str]]:
    """Returns (missing, present) binary names."""
    missing: list[str] = []
    present: list[str] = []
    for name in REQUIRED_BINARIES:
        if name == "ctags":
            if _is_universal_ctags():
                present.append(name)
            else:
                missing.append(name)
            continue
        if shutil.which(name):
            present.append(name)
        else:
            missing.append(name)
    return missing, present


def require_dependencies() -> None:
    """Hard-fail if any required binary is missing.

    Prints the missing binaries with per-platform install instructions
    and exits 1. Called at the start of every CLI command except `doctor`.
    """
    missing, _ = check_dependencies()
    if not missing:
        return
    p = detect_platform()
    click.echo("Error: tracer requires external binaries that are not installed.", err=True)
    click.echo("", err=True)
    click.echo("Missing:", err=True)
    for name in missing:
        instruction = REQUIRED_BINARIES[name].get(p, REQUIRED_BINARIES[name]["linux"])
        click.echo(f"  {name}  →  {instruction}", err=True)
    click.echo("", err=True)
    click.echo("Run `trace doctor` for full diagnostics.", err=True)
    sys.exit(1)
