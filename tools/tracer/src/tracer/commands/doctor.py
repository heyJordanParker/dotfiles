"""`trace doctor` — verify required external binaries."""

from __future__ import annotations

import sys

import click

from tracer.deps import REQUIRED_BINARIES, check_dependencies, detect_platform


@click.command()
def command() -> None:
    """Verify required external binaries are installed."""
    missing, present = check_dependencies()
    p = detect_platform()
    click.echo(f"Platform: {p}")
    click.echo("")
    if present:
        click.echo("Found:")
        for name in present:
            click.echo(f"  ✓ {name}")
        click.echo("")
    if missing:
        click.echo("Missing:")
        for name in missing:
            instruction = REQUIRED_BINARIES[name].get(p, REQUIRED_BINARIES[name]["linux"])
            click.echo(f"  ✗ {name}")
            click.echo(f"    install: {instruction}")
        sys.exit(1)
    click.echo("All required binaries installed.")
